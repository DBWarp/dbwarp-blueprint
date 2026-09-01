struct PendingCompressionSample {
    ticket: CompressionTicket,
    submitted_at: Instant,
    column_lengths: Vec<Option<(u64, u64)>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
}

async fn sample_compression(
    conn: &mut mysql_async::Conn,
    t: &TableRow,
    table_columns: &[ColumnRow],
    sample_rows: u64,
    length_fidelity: LengthFidelity,
    compression_pool: &CompressionWorkerPool,
    audit: &mut AuditLog,
) -> Result<Option<PendingCompressionSample>> {
    if table_columns.is_empty() {
        return Ok(None);
    }
    let qname = format!(
        "`{}`.`{}`",
        t.schema_name.replace('`', "``"),
        t.table_name.replace('`', "``")
    );
    let (bounded_rows, cell_char_limit) =
        crate::engine_common::live_sample_budget(sample_rows, table_columns.len(), 4);
    let (_, cell_byte_limit) =
        crate::engine_common::live_sample_budget(sample_rows, table_columns.len(), 1);
    let projection = table_columns
        .iter()
        .map(|column| {
            let name = quote_mysql_ident(&column.col_name);
            match column.col_type.as_str() {
                "binary" => format!("LEFT({name}, {cell_byte_limit})"),
                "text" | "json" | "user-defined" => {
                    format!("LEFT({name}, {cell_char_limit})")
                }
                _ => name,
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {projection} FROM {qname} LIMIT {bounded_rows}");
    let started = Instant::now();
    let rows: Vec<mysql_async::Row> = conn
        .query(sql.clone())
        .await
        .with_context(|| format!("sampling {qname}"))?;
    if rows.is_empty() {
        return Ok(None);
    }
    audit.record_query(
        "SELECT bounded projection FROM <table> LIMIT N (compression sample; server-side cell cap)",
        elapsed_ms(started),
        rows.len() as u64,
    );

    // Encode rows using `dbwarp-blueprint-rowframe-v1`. Each cell is
    // tagged via `encode_mysql_cell` based on column metadata + the
    // value variant. The text protocol used by `conn.query(...)`
    // returns most values as `Value::Bytes` carrying the column's
    // wire-format bytes (textual for numerics; UTF-8 / charset bytes
    // for text; raw bytes for BLOB-with-binary-charset). This makes
    // the compression measurement reflect actual MySQL wire-format byte
    // distributions. Tagged, length-prefixed cells keep binary and complex
    // values distinct from genuinely empty fields.
    let column_metadata: Vec<(MyColumnType, u16)> = rows[0]
        .columns_ref()
        .iter()
        .map(|c| (c.column_type(), c.character_set()))
        .collect();

    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut row_ranges: Vec<(usize, usize)> = Vec::with_capacity(rows.len());
    let mut column_bufs: Vec<Vec<u8>> = Vec::new();
    let mut column_payload_lengths: Vec<Vec<u64>> = Vec::new();
    let mut cardinality_accumulators: Vec<sample_encode::CardinalityAccumulator> = Vec::new();

    for r in &rows {
        let n_cols = r.columns_ref().len();
        if column_bufs.is_empty() {
            column_bufs = vec![Vec::new(); n_cols];
            column_payload_lengths = vec![Vec::new(); n_cols];
            cardinality_accumulators =
                vec![sample_encode::CardinalityAccumulator::default(); n_cols];
        }
        // Build owned (TypeTag, Vec<u8>) per cell, then borrow for the encoder.
        let cells_owned: Vec<(TypeTag, Vec<u8>)> = (0..n_cols)
            .map(|idx| {
                let v_ref = r.as_ref(idx).unwrap_or(&Value::NULL);
                let (col_type, charset) = column_metadata
                    .get(idx)
                    .copied()
                    .unwrap_or((MyColumnType::MYSQL_TYPE_NULL, 0));
                encode_mysql_cell(col_type, charset, v_ref)
            })
            .collect();
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
        .map(|lengths| sampled_column_length_stats(lengths, length_fidelity))
        .collect();
    let cardinalities = cardinality_accumulators
        .iter()
        .map(|accumulator| {
            accumulator.finish(
                t.rows_estimate,
                "LIMIT N (MySQL; server-side cell cap)",
                true,
                "natural_pk_order_no_native_tablesample+server_side_cell_cap",
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
            sample_method: "LIMIT N (MySQL; server-side cell cap)".to_string(),
            sampled_with_bias: true,
            bias_reason: "natural_pk_order_no_native_tablesample+server_side_cell_cap".to_string(),
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

fn sampled_column_length_stats(
    mut lengths: Vec<u64>,
    length_fidelity: LengthFidelity,
) -> Option<(u64, u64)> {
    if lengths.is_empty() {
        return None;
    }
    let average = lengths.iter().copied().sum::<u64>() / lengths.len() as u64;
    lengths.sort_unstable();
    let p95_index = ((lengths.len() * 95).div_ceil(100)).saturating_sub(1);
    let p95 = lengths[p95_index];
    Some(match length_fidelity {
        LengthFidelity::Exact => (average, p95),
        LengthFidelity::Balanced => {
            let rounded_average = format::round_len_relative(average).max(u64::from(average > 0));
            let rounded_p95 = format::round_len_relative(p95).max(rounded_average);
            (rounded_average, rounded_p95)
        }
        LengthFidelity::Strict => {
            let rounded_average = format::round_len_avg(average).max(u64::from(average > 0));
            let rounded_p95 = format::round_len_p95(p95).max(rounded_average);
            (rounded_average, rounded_p95)
        }
    })
}

fn blueprint_length(value: u64, length_fidelity: LengthFidelity) -> u64 {
    if value == 0 || length_fidelity.preserves_structure() {
        value
    } else {
        format::round_len_avg(value).max(1)
    }
}

fn blueprint_prefix_length(value: u64, length_fidelity: LengthFidelity) -> u64 {
    if value == 0 || length_fidelity.preserves_structure() {
        value
    } else {
        // Never round an index prefix upward: doing so can create a key wider
        // than the source index and can cross InnoDB's key-byte ceiling.
        ((value / 10) * 10).max(1)
    }
}

fn is_style_candidate_mysql(col: &ColumnRow) -> bool {
    matches!(col.col_type.as_str(), "text" | "json")
}

async fn peek_column_style(
    conn: &mut mysql_async::Conn,
    table: &TableRow,
    col: &ColumnRow,
) -> Result<&'static str> {
    let qname = format!(
        "{}.{}",
        quote_mysql_ident(&table.schema_name),
        quote_mysql_ident(&table.table_name)
    );
    let qcol = quote_mysql_ident(&col.col_name);
    let per_row_chars = (STYLE_PEEK_BYTES / 32 / 4).max(1);
    let sql = format!(
        "SELECT LEFT(CAST({qcol} AS CHAR), {per_row_chars}) FROM {qname} LIMIT 32"
    );
    let rows: Vec<mysql_async::Row> = conn
        .query(sql)
        .await
        .with_context(|| format!("sampling style for column {} on {}", col.ordinal, qname))?;
    let mut buf: Vec<u8> = Vec::with_capacity(STYLE_PEEK_BYTES);
    for row in rows {
        if buf.len() >= STYLE_PEEK_BYTES {
            break;
        }
        let value = row.as_ref(0).unwrap_or(&Value::NULL);
        append_mysql_value_for_style(value, &mut buf);
        if !buf.ends_with(b"\n") {
            buf.push(b'\n');
        }
        if buf.len() > STYLE_PEEK_BYTES {
            buf.truncate(STYLE_PEEK_BYTES);
            break;
        }
    }
    Ok(style::classify(&buf))
}

fn append_mysql_value_for_style(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::NULL => {}
        Value::Bytes(bytes) => out.extend_from_slice(bytes),
        Value::Int(v) => out.extend_from_slice(v.to_string().as_bytes()),
        Value::UInt(v) => out.extend_from_slice(v.to_string().as_bytes()),
        Value::Float(v) => out.extend_from_slice(v.to_string().as_bytes()),
        Value::Double(v) => out.extend_from_slice(v.to_string().as_bytes()),
        Value::Date(y, mo, d, h, mi, s, _us) => {
            out.extend_from_slice(format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}").as_bytes())
        }
        Value::Time(neg, days, h, mi, s, _us) => out.extend_from_slice(
            format!(
                "{}{days:03}d {h:02}:{mi:02}:{s:02}",
                if *neg { "-" } else { "" }
            )
            .as_bytes(),
        ),
    }
}

fn quote_mysql_ident(raw: &str) -> String {
    format!("`{}`", raw.replace('`', "``"))
}
