struct PendingCompressionSample {
    ticket: CompressionTicket,
    submitted_at: Instant,
    column_lengths: Vec<Option<(u64, u64)>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
    payload_profiles: Vec<String>,
    complete_source_rows: Option<u64>,
}

fn mssql_complete_row_read(
    returned_rows: usize,
    requested_rows: u64,
    has_security_filter: bool,
) -> bool {
    returned_rows < requested_rows as usize && !has_security_filter
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MssqlSampleProjectionLimit {
    byte_limit: usize,
    char_limit: usize,
}

// Tiberius materializes the complete first result before the probe encoder can
// discard it. Each source column is projected as value + sampled DATALENGTH +
// original DATALENGTH, so bounding only the eventual 16 MiB encoded probe is
// insufficient for wide schemas. Keep the discovery pass smaller; an exact
// observed-width retry below spends the full probe budget on fewer rows.
const MSSQL_INITIAL_SAMPLE_MAX_ROWS: u64 = 32_768;
const MSSQL_INITIAL_SAMPLE_PAYLOAD_DIVISOR: usize = 2;

fn mssql_is_projected_variable_column(column: &ColumnRow) -> bool {
    matches!(
        column.native_type.as_str(),
        "binary"
            | "varbinary"
            | "image"
            | "char"
            | "varchar"
            | "text"
            | "nchar"
            | "nvarchar"
            | "ntext"
            | "xml"
            | "user-defined"
    )
}

fn mssql_fixed_sample_payload_reserve(column: &ColumnRow) -> usize {
    match column.col_type.as_str() {
        "boolean" => 1,
        "integer" => usize::try_from(column.numeric_precision)
            .unwrap_or(64)
            .saturating_add(2)
            .max(8),
        "numeric" => usize::try_from(column.numeric_precision)
            .unwrap_or(64)
            .saturating_add(4)
            .max(16),
        "float" => 32,
        "date" => 16,
        "time" | "timestamp" => 64,
        "uuid" => 36,
        _ => 64,
    }
}

fn mssql_sample_bytes_per_character(column: &ColumnRow) -> usize {
    if column.col_type == "binary" {
        return 1;
    }
    // LEFT counts characters while the source and compression probe are
    // byte-budgeted. Four is the safe upper bound for one Unicode scalar in
    // UTF-8 or one supplementary character in UTF-16LE.
    4
}

/// Floor the prefix so a pathologically small per-column budget cannot emit
/// an always-empty LEFT(col, 0) sample. The byte limit re-covers the floored
/// prefix's worst-case width so the row re-clamp stays honest.
fn mssql_floored_projection_limit(
    column: &ColumnRow,
    byte_limit: usize,
) -> MssqlSampleProjectionLimit {
    let bytes_per_character = mssql_sample_bytes_per_character(column);
    let char_limit = (byte_limit / bytes_per_character).max(1);
    MssqlSampleProjectionLimit {
        byte_limit: byte_limit.max(char_limit.saturating_mul(bytes_per_character)),
        char_limit,
    }
}

fn mssql_rows_within_sample_budget(
    planned_rows: u64,
    columns: &[ColumnRow],
    limits: &[MssqlSampleProjectionLimit],
) -> u64 {
    let payloads = columns
        .iter()
        .zip(limits)
        .map(|(column, limit)| {
            if mssql_is_projected_variable_column(column) {
                limit.byte_limit as u64
            } else {
                mssql_fixed_sample_payload_reserve(column) as u64
            }
        })
        .collect::<Vec<_>>();
    crate::engine_common::rows_within_sample_budget(planned_rows, &payloads)
}

fn mssql_sample_projection_budget(
    requested_rows: u64,
    columns: &[ColumnRow],
) -> Result<(u64, Vec<MssqlSampleProjectionLimit>)> {
    let shapes = columns
        .iter()
        .map(|column| {
            if mssql_is_projected_variable_column(column) {
                dbwarp_blueprint_core::TransferProbeColumnShape::Variable {
                    declared_max_bytes: column.declared_max_bytes,
                }
            } else {
                dbwarp_blueprint_core::TransferProbeColumnShape::Fixed {
                    payload_reserve: mssql_fixed_sample_payload_reserve(column) as u64,
                }
            }
        })
        .collect::<Vec<_>>();
    let plan = crate::engine_common::bounded_projection_plan(
        requested_rows.min(MSSQL_INITIAL_SAMPLE_MAX_ROWS),
        &shapes,
        false,
    )?;
    let limits = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            if mssql_is_projected_variable_column(column) {
                let byte_limit = usize::try_from(plan.variable_byte_limits[index])
                    .unwrap_or(usize::MAX)
                    .div_ceil(MSSQL_INITIAL_SAMPLE_PAYLOAD_DIVISOR)
                    .max(1);
                // DATALENGTH evidence below drives a bounded retry;
                // original lengths, not prefix lengths, describe the source.
                mssql_floored_projection_limit(column, byte_limit)
            } else {
                MssqlSampleProjectionLimit {
                    byte_limit: 0,
                    char_limit: 0,
                }
            }
        })
        .collect::<Vec<_>>();
    let rows = mssql_rows_within_sample_budget(plan.sample_rows, columns, &limits);
    Ok((rows, limits))
}

fn mssql_sample_value_expression(
    column: &ColumnRow,
    limit: MssqlSampleProjectionLimit,
) -> (String, String) {
    let name = format!("[{}]", column.col_name.replace(']', "]]"));
    match column.native_type.as_str() {
        "binary" | "varbinary" | "image" => (
            format!(
                "SUBSTRING(CONVERT(varbinary(max), {name}), 1, {})",
                limit.byte_limit
            ),
            format!("CONVERT(varbinary(max), {name})"),
        ),
        "char" | "varchar" | "nchar" | "nvarchar" => {
            (format!("LEFT({name}, {})", limit.char_limit), name)
        }
        "text" => (
            format!("LEFT(CONVERT(varchar(max), {name}), {})", limit.char_limit),
            format!("CONVERT(varchar(max), {name})"),
        ),
        "ntext" | "xml" | "user-defined" => (
            format!(
                "LEFT(TRY_CONVERT(nvarchar(max), {name}), {})",
                limit.char_limit
            ),
            format!("TRY_CONVERT(nvarchar(max), {name})"),
        ),
        _ => (name.clone(), name),
    }
}

fn mssql_sample_projection(columns: &[ColumnRow], limits: &[MssqlSampleProjectionLimit]) -> String {
    columns
        .iter()
        .zip(limits)
        .flat_map(|(column, limit)| {
            let (sampled, original) = mssql_sample_value_expression(column, *limit);
            [
                sampled.clone(),
                format!("CONVERT(bigint, DATALENGTH({sampled}))"),
                format!("CONVERT(bigint, DATALENGTH({original}))"),
            ]
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn mssql_observed_octet_length(value: &ColumnData<'_>) -> Option<u64> {
    match value {
        ColumnData::U8(Some(value)) => Some(u64::from(*value)),
        ColumnData::I16(Some(value)) => u64::try_from(*value).ok(),
        ColumnData::I32(Some(value)) => u64::try_from(*value).ok(),
        ColumnData::I64(Some(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn mssql_adaptive_projection_from_observed_lengths(
    rows: &[tiberius::Row],
    columns: &[ColumnRow],
    requested_rows: u64,
) -> Result<Option<(u64, Vec<MssqlSampleProjectionLimit>)>> {
    let (observed_max, truncated_columns) = mssql_sample_observed_lengths(rows, columns);
    if !truncated_columns.iter().any(|truncated| *truncated) {
        return Ok(None);
    }
    mssql_adaptive_projection_budget(columns, &observed_max, requested_rows).map(Some)
}

fn mssql_sample_observed_lengths(
    rows: &[tiberius::Row],
    columns: &[ColumnRow],
) -> (Vec<u64>, Vec<bool>) {
    let mut observed_max = vec![0_u64; columns.len()];
    let mut truncated = vec![false; columns.len()];
    for row in rows {
        let cells = row.cells().collect::<Vec<_>>();
        for (column_index, column) in columns.iter().enumerate() {
            if !mssql_is_projected_variable_column(column) {
                continue;
            }
            let base = column_index.saturating_mul(3);
            let sampled_bytes = cells
                .get(base.saturating_add(1))
                .and_then(|(_, value)| mssql_observed_octet_length(value))
                .unwrap_or(0);
            let original_bytes = cells
                .get(base.saturating_add(2))
                .and_then(|(_, value)| mssql_observed_octet_length(value))
                .unwrap_or(sampled_bytes);
            observed_max[column_index] = observed_max[column_index].max(original_bytes);
            truncated[column_index] |= sampled_bytes < original_bytes;
        }
    }
    (observed_max, truncated)
}

fn mssql_adaptive_projection_budget(
    columns: &[ColumnRow],
    observed_max: &[u64],
    requested_rows: u64,
) -> Result<(u64, Vec<MssqlSampleProjectionLimit>)> {
    let shapes = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            if mssql_is_projected_variable_column(column) {
                let bytes_per_character = if matches!(
                    column.native_type.as_str(),
                    "binary" | "varbinary" | "image"
                ) {
                    1
                } else {
                    4
                };
                dbwarp_blueprint_core::TransferProbeColumnShape::Variable {
                    declared_max_bytes: observed_max[index]
                        .max(1)
                        .saturating_mul(bytes_per_character),
                }
            } else {
                dbwarp_blueprint_core::TransferProbeColumnShape::Fixed {
                    payload_reserve: mssql_fixed_sample_payload_reserve(column) as u64,
                }
            }
        })
        .collect::<Vec<_>>();
    let plan = crate::engine_common::bounded_projection_plan(requested_rows, &shapes, true)?;
    let limits = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            if mssql_is_projected_variable_column(column) {
                // Same floors as the initial projection: never emit an
                // always-empty LEFT(col, 0) prefix on the retry either.
                let byte_limit = (plan.variable_byte_limits[index] as usize).max(1);
                mssql_floored_projection_limit(column, byte_limit)
            } else {
                MssqlSampleProjectionLimit {
                    byte_limit: 0,
                    char_limit: 0,
                }
            }
        })
        .collect::<Vec<_>>();
    let rows = mssql_rows_within_sample_budget(plan.sample_rows, columns, &limits);
    Ok((rows, limits))
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
    let (bounded_rows, projection_limits) =
        mssql_sample_projection_budget(sample_rows, table_columns)?;
    let projection = mssql_sample_projection(table_columns, &projection_limits);
    let sql =
        format!("SELECT TOP ({bounded_rows}) {projection} FROM {qname} ORDER BY (SELECT NULL)");
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
    let mut rows = match stream.into_first_result().await {
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

    let mut sample_method =
        "TOP N bounded projection with DATALENGTH; native UTF-16LE for nvarchar".to_string();
    let mut bias_reason = "natural_storage_order_no_native_random_sample".to_string();
    let mut complete_row_read =
        mssql_complete_row_read(rows.len(), bounded_rows, t.has_security_filter);
    if let Some((retry_rows, retry_limits)) = mssql_adaptive_projection_from_observed_lengths(
        rows.as_slice(),
        table_columns,
        bounded_rows,
    )? {
        let retry_projection = mssql_sample_projection(table_columns, &retry_limits);
        let retry_sql = format!(
            "SELECT TOP ({retry_rows}) {retry_projection} FROM {qname} ORDER BY (SELECT NULL)"
        );
        let retry_started = Instant::now();
        drop(rows);
        rows = client
            .simple_query(retry_sql)
            .await
            .with_context(|| format!("retrying bounded compression sample for {qname}"))?
            .into_first_result()
            .await
            .with_context(|| format!("reading retried bounded compression sample for {qname}"))?;
        complete_row_read =
            mssql_complete_row_read(rows.len(), retry_rows, t.has_security_filter);
        audit.record_query(
            "SELECT TOP N adaptive byte-bounded projection FROM <table> (observed DATALENGTH)",
            elapsed_ms(retry_started),
            rows.len() as u64,
        );
        sample_method.push_str("; adaptive byte-bounded retry from observed DATALENGTH");
        bias_reason.push_str("+adaptive_observed_datalength_retry");
    }
    let (_, column_value_truncated) = mssql_sample_observed_lengths(&rows, table_columns);
    if column_value_truncated.iter().any(|truncated| *truncated) {
        crate::engine_common::record_sample_prefix_bias(&mut sample_method, &mut bias_reason);
    }
    if rows.is_empty() {
        return Ok(None);
    }

    // Encode rows using the transient compression-probe representation. Per cell, we use
    // `encode_mssql_cell` to choose the right TypeTag: the load-
    // bearing case is nvarchar / nchar / nText, which we re-encode as
    // UTF-16LE so the measurement preserves the source type's byte width and
    // repeated-byte distribution.
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut row_ranges: Vec<(usize, usize)> = Vec::with_capacity(rows.len());
    let mut column_bufs: Vec<Vec<u8>> = Vec::new();
    let mut column_payload_lengths: Vec<Vec<u64>> = Vec::new();
    let mut cardinality_accumulators: Vec<sample_encode::CardinalityAccumulator> = Vec::new();
    let mut payload_profile_accumulators: Vec<dbwarp_blueprint_core::PayloadProfileAccumulator> =
        Vec::new();
    for r in &rows {
        // Build owned payload bytes per cell so the Cell<'_> references
        // don't outlive the row's data borrow.
        let projected_cells = r.cells().collect::<Vec<_>>();
        if projected_cells.len() != table_columns.len().saturating_mul(3) {
            bail!(
                "SQL Server compression sample returned {} fields for {} source columns",
                projected_cells.len(),
                table_columns.len()
            );
        }
        let cells_owned: Vec<(TypeTag, Vec<u8>)> = projected_cells
            .iter()
            .step_by(3)
            .map(|(col, cd)| encode_mssql_cell(col.column_type(), cd))
            .collect();
        if column_bufs.is_empty() {
            column_bufs = vec![Vec::new(); cells_owned.len()];
            column_payload_lengths = vec![Vec::new(); cells_owned.len()];
            cardinality_accumulators =
                vec![sample_encode::CardinalityAccumulator::default(); cells_owned.len()];
            payload_profile_accumulators = vec![
                dbwarp_blueprint_core::PayloadProfileAccumulator::default();
                cells_owned.len()
            ];
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
        for (col_idx, (tag, _payload)) in cells_owned.iter().enumerate() {
            if *tag != TypeTag::Null {
                let base = col_idx.saturating_mul(3);
                if let Some(length) = projected_cells
                    .get(base.saturating_add(2))
                    .and_then(|(_, value)| mssql_observed_octet_length(value))
                {
                    column_payload_lengths[col_idx].push(length);
                }
            }
        }
        for (col_idx, cell) in cells.iter().enumerate() {
            if let Some(accumulator) = cardinality_accumulators.get_mut(col_idx) {
                accumulator.push(cell);
            }
            if cell.tag == TypeTag::BinaryRaw {
                if let (Some(accumulator), Some(payload)) =
                    (payload_profile_accumulators.get_mut(col_idx), cell.bytes)
                {
                    accumulator.observe_raw_binary(payload);
                }
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
    complete_row_read &= row_ranges.len() == rows.len();

    let column_lengths = column_payload_lengths
        .into_iter()
        .map(sampled_mssql_column_length_stats)
        .collect::<Vec<_>>();
    let cardinalities = cardinality_accumulators
        .iter()
        .enumerate()
        .map(|(index, accumulator)| {
            accumulator.finish_with_source_rows(
                complete_row_read
                    .then_some(row_ranges.len() as u64)
                    .or(t.row_count),
                complete_row_read,
                complete_row_read && !column_value_truncated[index],
                sample_method.as_str(),
                true,
                bias_reason.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let null_fractions = cardinality_accumulators
        .iter()
        .zip(cardinalities.iter())
        .map(|(accumulator, cardinality)| {
            accumulator.emitted_null_fraction(cardinality.as_ref())
        })
        .collect();
    let payload_profiles = payload_profile_accumulators
        .iter()
        .map(|accumulator| accumulator.style().to_string())
        .collect();

    let encoded_sample_rows = row_ranges.len() as u64;
    let submitted_at = Instant::now();
    let ticket = compression_pool
        .submit(PreparedCompressionSample {
            column_bytes: column_bufs,
            sample_rows: encoded_sample_rows,
            sample_method,
            sampled_with_bias: true,
            bias_reason,
            compression_chunk_bytes: Some(
                dbwarp_blueprint_core::TRANSFER_PROBE_STREAMING_COMPRESSION_CHUNK_BYTES,
            ),
        })
        .with_context(|| format!("submitting local compression work for {qname}"))?;
    audit.record_compression_job_submitted();

    Ok(Some(PendingCompressionSample {
        ticket,
        submitted_at,
        column_lengths,
        null_fractions,
        cardinalities,
        payload_profiles,
        complete_source_rows: complete_row_read.then_some(encoded_sample_rows),
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
