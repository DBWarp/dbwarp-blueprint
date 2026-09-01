struct PendingCompressionSample {
    ticket: CompressionTicket,
    submitted_at: Instant,
    column_lengths: Vec<Option<(u64, u64)>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
}

async fn sample_compression(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    t: &TableRow,
    table_id: &str,
    table_columns: &[ColumnRow],
    sample_rows: u64,
    compression_pool: &CompressionWorkerPool,
    audit: &mut AuditLog,
) -> Result<Option<PendingCompressionSample>> {
    if table_columns.is_empty() {
        return Ok(None);
    }
    let qname = format!(
        "[{}].[{}]",
        t.schema_name.replace(']', "]]"),
        t.table_name.replace(']', "]]"),
    );
    // SQL Server: TABLESAMPLE SYSTEM is page-lumpy on small tables (same
    // problem PG has). Use OFFSET 0 ROWS FETCH NEXT N ROWS ONLY ordered
    // by some key. Without knowing the PK at this point, ORDER BY
    // (SELECT NULL) is the SQL Server idiom for "no order required".
    let (bounded_rows, cell_char_limit) =
        crate::engine_common::live_sample_budget(sample_rows, table_columns.len(), 4);
    let (_, cell_byte_limit) =
        crate::engine_common::live_sample_budget(sample_rows, table_columns.len(), 1);
    let projection = table_columns
        .iter()
        .map(|column| {
            let name = format!("[{}]", column.col_name.replace(']', "]]"));
            match column.native_type.as_str() {
                "binary" | "varbinary" | "image" => format!(
                    "SUBSTRING(CONVERT(varbinary(max), {name}), 1, {cell_byte_limit})"
                ),
                "char" | "varchar" | "nchar" | "nvarchar" => {
                    format!("LEFT({name}, {cell_char_limit})")
                }
                "text" => format!(
                    "LEFT(CONVERT(varchar(max), {name}), {cell_char_limit})"
                ),
                "ntext" | "xml" => format!(
                    "LEFT(CONVERT(nvarchar(max), {name}), {cell_char_limit})"
                ),
                "user-defined" => format!(
                    "LEFT(TRY_CONVERT(nvarchar(max), {name}), {cell_char_limit})"
                ),
                _ => name,
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT TOP ({bounded_rows}) {projection} FROM {qname} ORDER BY (SELECT NULL)"
    );
    let started = Instant::now();
    let stream = match client.simple_query(sql.clone()).await {
        Ok(s) => s,
        Err(_) => {
            let redacted = crate::i18n::format(
                "engine.driver_detail_redacted",
                &[("target", table_id.to_string())],
            );
            let detail = crate::i18n::format(
                "engine.sample_query_failed",
                &[
                    ("code", "DBP1407W".to_string()),
                    ("table", table_id.to_string()),
                    ("error", redacted),
                ],
            );
            tracing_eprintln(detail.clone());
            audit.record_warning("DBP1407W", detail);
            return Ok(None);
        }
    };
    let rows = match stream.into_first_result().await {
        Ok(rs) => rs,
        Err(_) => {
            let redacted = crate::i18n::format(
                "engine.driver_detail_redacted",
                &[("target", table_id.to_string())],
            );
            let detail = crate::i18n::format(
                "engine.sample_stream_failed",
                &[
                    ("code", "DBP1407W".to_string()),
                    ("table", table_id.to_string()),
                    ("error", redacted),
                ],
            );
            tracing_eprintln(detail.clone());
            audit.record_warning("DBP1407W", detail);
            return Ok(None);
        }
    };
    if rows.is_empty() {
        return Ok(None);
    }
    audit.record_query(
        "SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)",
        elapsed_ms(started),
        rows.len() as u64,
    );

    // Encode rows using `dbwarp-blueprint-rowframe-v1`. Per cell, we use
    // `encode_mssql_cell` to choose the right TypeTag — the load-
    // bearing case is nvarchar / nchar / nText, which we re-encode as
    // UTF-16LE so the compression measurement reflects the actual
    // wire/storage byte distribution that drives MSSQL's higher zstd
    // ratios on Unicode text columns.
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut row_ranges: Vec<(usize, usize)> = Vec::with_capacity(rows.len());
    let mut column_bufs: Vec<Vec<u8>> = Vec::new();
    let mut column_payload_lengths: Vec<Vec<u64>> = Vec::new();
    let mut cardinality_accumulators: Vec<sample_encode::CardinalityAccumulator> = Vec::new();
    for r in &rows {
        // Build owned payload bytes per cell so the Cell<'_> references
        // don't outlive the row's data borrow.
        let cells_owned: Vec<(TypeTag, Vec<u8>)> = r
            .cells()
            .map(|(col, cd)| encode_mssql_cell(col.column_type(), cd))
            .collect();
        if column_bufs.is_empty() {
            column_bufs = vec![Vec::new(); cells_owned.len()];
            column_payload_lengths = vec![Vec::new(); cells_owned.len()];
            cardinality_accumulators =
                vec![sample_encode::CardinalityAccumulator::default(); cells_owned.len()];
        }
        let cells: Vec<Cell<'_>> = cells_owned
            .iter()
            .map(|(tag, payload)| match *tag {
                TypeTag::Null => Cell::null(),
                t => Cell::new(t, payload.as_slice()),
            })
            .collect();
        let row_start = buf.len();
        let mut encoded_row = Vec::new();
        sample_encode::encode_row(&mut encoded_row, &cells)
            .with_context(|| format!("encoding sample row from {qname}"))?;
        if buf.len().saturating_add(encoded_row.len())
            > crate::engine_common::MAX_LIVE_TABLE_SAMPLE_BYTES
        {
            break;
        }
        for (col_idx, (tag, payload)) in cells_owned.iter().enumerate() {
            if *tag != TypeTag::Null {
                column_payload_lengths[col_idx].push(payload.len() as u64);
            }
        }
        for (col_idx, cell) in cells.iter().enumerate() {
            if let Some(accumulator) = cardinality_accumulators.get_mut(col_idx) {
                accumulator.push(cell);
            }
            if let Some(col_buf) = column_bufs.get_mut(col_idx) {
                sample_encode::encode_row(col_buf, std::slice::from_ref(cell))
                    .with_context(|| format!("encoding sample column from {qname}"))?;
            }
        }
        buf.extend_from_slice(&encoded_row);
        row_ranges.push((row_start, buf.len()));
    }
    if buf.is_empty() {
        return Ok(None);
    }
    let sample_bytes = buf.len() as u64;
    audit.record_encoded_sample_bytes(sample_bytes)?;

    let column_lengths = column_payload_lengths
        .into_iter()
        .map(sampled_mssql_column_length_stats)
        .collect();
    let cardinalities = cardinality_accumulators
        .iter()
        .map(|accumulator| {
            accumulator.finish(
                t.row_count,
                "TOP N bounded projection; UTF-16LE for nvarchar",
                true,
                "natural_storage_order_no_native_random_sample+server_side_cell_cap",
            )
        })
        .collect();
    let null_fractions = cardinality_accumulators
        .iter()
        .map(sample_encode::CardinalityAccumulator::null_fraction)
        .collect();

    let encoded_sample_rows = row_ranges.len() as u64;
    let submitted_at = Instant::now();
    let ticket = compression_pool
        .submit(PreparedCompressionSample {
            table_bytes: buf,
            row_ranges,
            column_bytes: column_bufs,
            sample_rows: encoded_sample_rows,
            sample_method: "TOP N bounded projection; UTF-16LE for nvarchar".to_string(),
            sampled_with_bias: true,
            bias_reason:
                "natural_storage_order_no_native_random_sample+server_side_cell_cap".to_string(),
        })
        .with_context(|| format!("submitting local compression work for {qname}"))?;
    audit.record_compression_job_submitted();

    Ok(Some(PendingCompressionSample {
        ticket,
        submitted_at,
        column_lengths,
        null_fractions,
        cardinalities,
    }))
}

fn sampled_mssql_column_length_stats(mut lengths: Vec<u64>) -> Option<(u64, u64)> {
    if lengths.is_empty() {
        return None;
    }
    let average = lengths.iter().copied().sum::<u64>() / lengths.len() as u64;
    lengths.sort_unstable();
    let p95_index = ((lengths.len() * 95).div_ceil(100)).saturating_sub(1);
    let p95 = lengths[p95_index];
    let rounded_average = format::round_len_relative(average).max(u64::from(average > 0));
    let rounded_p95 = format::round_len_relative(p95).max(rounded_average);
    Some((rounded_average, rounded_p95))
}

fn is_variable_length_mssql(native_type: &str) -> bool {
    matches!(
        native_type,
        "varchar" | "nvarchar" | "varbinary" | "text" | "ntext" | "image"
    )
}

fn is_style_candidate_mssql(col: &ColumnRow) -> bool {
    col.col_type == "text"
}

async fn peek_column_style_mssql(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    table: &TableRow,
    col: &ColumnRow,
) -> Result<&'static str> {
    let qname = format!(
        "[{}].[{}]",
        table.schema_name.replace(']', "]]"),
        table.table_name.replace(']', "]]")
    );
    let qcol = format!("[{}]", col.col_name.replace(']', "]]"));
    let per_row_chars = (STYLE_PEEK_BYTES / 32 / 2).max(1);
    let sql = format!(
        "SELECT TOP (32) LEFT(TRY_CONVERT(nvarchar(max), {qcol}), {per_row_chars}) FROM {qname} ORDER BY (SELECT NULL)"
    );
    let rows = client
        .simple_query(sql)
        .await
        .with_context(|| format!("sampling style for column {} on {}", col.ordinal, qname))?
        .into_first_result()
        .await
        .unwrap_or_default();
    let mut buf: Vec<u8> = Vec::with_capacity(STYLE_PEEK_BYTES);
    for row in rows {
        if buf.len() >= STYLE_PEEK_BYTES {
            break;
        }
        if let Some((_meta, data)) = row.cells().next() {
            append_mssql_value_for_style(data, &mut buf);
            if !buf.ends_with(b"\n") {
                buf.push(b'\n');
            }
            if buf.len() > STYLE_PEEK_BYTES {
                buf.truncate(STYLE_PEEK_BYTES);
                break;
            }
        }
    }
    Ok(style::classify(&buf))
}

fn append_mssql_value_for_style(data: &ColumnData<'_>, out: &mut Vec<u8>) {
    match data {
        ColumnData::String(Some(s)) => out.extend_from_slice(s.as_bytes()),
        ColumnData::Xml(Some(x)) => out.extend_from_slice(format!("{x:?}").as_bytes()),
        other => {
            let (_tag, payload) = encode_mssql_cell(ColumnType::NVarchar, other);
            out.extend_from_slice(&payload);
        }
    }
}
