struct PendingCompressionSample {
    ticket: CompressionTicket,
    submitted_at: Instant,
    column_lengths: Vec<Option<(u64, u64)>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
    payload_profiles: Vec<String>,
}

async fn sample_compression(
    client: &tokio_postgres::Client,
    table: &TableRow,
    table_id: &str,
    table_columns: &[ColumnRow],
    sample_rows: u64,
    length_fidelity: LengthFidelity,
    compression_pool: &CompressionWorkerPool,
    audit: &mut AuditLog,
) -> Result<Option<PendingCompressionSample>> {
    // Build qualified, quoted table name.
    let qname = format!(
        "\"{}\".\"{}\"",
        table.schema_name.replace('"', "\"\""),
        table.table_name.replace('"', "\"\"")
    );

    if table_columns.is_empty() {
        return Ok(None);
    }
    // Budget the projection in bytes. Text values reserve four bytes per
    // character for UTF-8; bytea values can use the full per-cell byte budget
    // because they are decoded back to raw bytes before probe encoding.
    let (bounded_rows, cell_byte_limit) =
        crate::engine_common::live_sample_budget(sample_rows, table_columns.len(), 1);
    let cell_char_limit = (cell_byte_limit / 4).max(1);
    let projection = table_columns
        .iter()
        .flat_map(|column| {
            let name = format!("\"{}\"", column.attname.replace('"', "\"\""));
            let tag = type_tag_for_pg_str(&column.type_str);
            let observed_length = match tag {
                TypeTag::BinaryRaw => format!("octet_length({name})"),
                TypeTag::VectorBinary => format!("octet_length(vector_send({name}))"),
                _ => format!("octet_length({name}::text)"),
            };
            let sampled_value = match tag {
                TypeTag::BinaryRaw => {
                // simple_query returns text fields. PostgreSQL's ordinary
                // bytea text output is hexadecimal text, which must not be
                // mistaken for the sampled binary payload. Ask for explicit
                // hex and decode it locally so both cardinality and
                // compression observe the real bytes.
                    format!(
                        "encode(substring({name} FROM 1 FOR {cell_byte_limit}), 'hex')"
                    )
                }
                TypeTag::VectorBinary => format!(
                    "encode(substring(vector_send({name}) FROM 1 FOR {cell_byte_limit}), 'hex')"
                ),
                _ => format!("LEFT({name}::text, {cell_char_limit})"),
            };
            [sampled_value, observed_length]
        })
        .collect::<Vec<_>>()
        .join(", ");

    // Map column ordinals to TypeTags from the catalog scan we already
    // ran in run(). We require the columns in attnum order — the
    // catalog query orders them that way — so position N in
    // `table_columns` corresponds to column N in the SELECT * result.
    let column_tags: Vec<TypeTag> = table_columns
        .iter()
        .map(|c| type_tag_for_pg_str(&c.type_str))
        .collect();

    // Use an estimate-aware TABLESAMPLE percentage plus LIMIT. A fixed 0.1%
    // sample systematically underfills moderate tables (for example, 375k
    // rows yields only about 375 rows for a requested 1,000-row sample). Four
    // times the requested expected rows absorbs page-level variance while the
    // LIMIT and resident-memory budget retain the existing safety bounds.
    //
    // We use the *simple query* protocol (not the extended `client.query`
    // path) so the server returns column values in TEXT format. The
    // extended-query path opportunistically uses BINARY format for types whose
    // `FromSql` implementation accepts it. Text format provides one stable,
    // driver-independent value representation for this measurement contract.
    let sample_percent = pg_table_sample_percent(table.reltuples, bounded_rows);
    let sql = format!(
        "SELECT {projection} FROM {qname} TABLESAMPLE SYSTEM ({sample_percent:.6}) REPEATABLE (0) LIMIT {bounded_rows}"
    );
    let started = Instant::now();
    let mut sampled_with_bias = true;
    let mut bias_reason = "server_side_cell_cap".to_string();
    let mut sample_method = pg_sample_method(false).to_string();
    let mut primary_error = None;
    let mut messages = match client.simple_query(&sql).await {
        Ok(m) => {
            let elapsed = elapsed_ms(started);
            let row_count = m
                .iter()
                .filter(|message| matches!(message, SimpleQueryMessage::Row(_)))
                .count() as u64;
            audit.record_query(
                "adaptive TABLESAMPLE on a single user table (compression sample)",
                elapsed,
                row_count,
            );
            m
        }
        Err(error) => {
            primary_error = Some(error);
            Vec::new()
        }
    };
    let mut rows: Vec<tokio_postgres::SimpleQueryRow> = messages
        .drain(..)
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(r) => Some(r),
            _ => None,
        })
        .collect();
    if rows.len() < bounded_rows as usize {
        let fallback = format!("SELECT {projection} FROM {qname} LIMIT {bounded_rows}");
        let fallback_started = Instant::now();
        match client.simple_query(&fallback).await {
            Ok(mut fb) => {
                let fallback_elapsed = elapsed_ms(fallback_started);
                rows = fb
                    .drain(..)
                    .filter_map(|m| match m {
                        SimpleQueryMessage::Row(r) => Some(r),
                        _ => None,
                    })
                    .collect();
                audit.record_query(
                    "unordered LIMIT fallback on a single user table (compression sample)",
                    fallback_elapsed,
                    rows.len() as u64,
                );
                sampled_with_bias = true;
                bias_reason =
                    "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
                        .to_string();
                sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)".to_string();
            }
            Err(_) if !rows.is_empty() => {
                sampled_with_bias = true;
                bias_reason =
                    "underfilled_adaptive_TABLESAMPLE+failed_limit_fallback+server_side_cell_cap"
                        .to_string();
                sample_method = pg_sample_method(true).to_string();
                record_pg_partial_sample_warning(table_id, rows.len(), audit);
            }
            Err(fallback_error) => {
                return Err(fallback_error).with_context(|| {
                    if let Some(primary_error) = &primary_error {
                        format!(
                            "adaptive TABLESAMPLE query failed ({primary_error}); fallback LIMIT query also failed for {qname}"
                        )
                    } else {
                        format!(
                            "underfilled adaptive TABLESAMPLE returned no usable rows; fallback LIMIT query also failed for {qname}"
                        )
                    }
                });
            }
        }
    }
    if rows.is_empty() {
        return Ok(None);
    }
    // Encode rows using the transient compression-probe representation. Each column carries
    // its TEXT-format bytes (UTF-8 for everything tokio-postgres text-mode
    // returns) plus a type tag from the catalog scan. The tagged,
    // length-prefixed representation prevents
    // non-text columns from collapsing into ambiguous empty fields. The full
    // encoding contract and regression tests live in `src/sample_encode.rs`.
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut row_ranges: Vec<(usize, usize)> = Vec::with_capacity(rows.len());
    let mut column_bufs: Vec<Vec<u8>> = Vec::new();
    let mut column_payload_lengths: Vec<Vec<u64>> = Vec::new();
    let mut cardinality_accumulators: Vec<sample_encode::CardinalityAccumulator> = Vec::new();
    let mut payload_profile_accumulators: Vec<
        dbwarp_blueprint_core::PayloadProfileAccumulator,
    > = Vec::new();

    for r in &rows {
        let n_cols = table_columns.len();
        if column_bufs.is_empty() {
            column_bufs = vec![Vec::new(); n_cols];
            column_payload_lengths = vec![Vec::new(); n_cols];
            cardinality_accumulators =
                vec![sample_encode::CardinalityAccumulator::default(); n_cols];
            payload_profile_accumulators =
                vec![dbwarp_blueprint_core::PayloadProfileAccumulator::default(); n_cols];
        }
        let mut sampled_payloads: Vec<Option<std::borrow::Cow<'_, [u8]>>> =
            Vec::with_capacity(n_cols);
        for col_idx in 0..n_cols {
            let value_idx = col_idx.saturating_mul(2);
            let length_idx = value_idx.saturating_add(1);
            let cell_text: Option<&str> = r.get(value_idx);
            if let Some(length) = r.get(length_idx) {
                column_payload_lengths[col_idx].push(length.parse::<u64>().with_context(|| {
                    format!("decoding sampled PostgreSQL column length from {qname}")
                })?);
            }
            match cell_text {
                Some(s) => {
                    let tag = column_tags
                        .get(col_idx)
                        .copied()
                        .unwrap_or(TypeTag::UnknownText);
                    let payload = if matches!(tag, TypeTag::BinaryRaw | TypeTag::VectorBinary) {
                        std::borrow::Cow::Owned(decode_pg_hex_sample(s).with_context(|| {
                            format!("decoding sampled PostgreSQL bytea payload from {qname}")
                        })?)
                    } else {
                        std::borrow::Cow::Borrowed(s.as_bytes())
                    };
                    sampled_payloads.push(Some(payload));
                }
                None => sampled_payloads.push(None),
            }
        }
        let cells = sampled_payloads
            .iter()
            .enumerate()
            .map(|(col_idx, payload)| match payload {
                Some(payload) => Cell::new(
                    column_tags
                        .get(col_idx)
                        .copied()
                        .unwrap_or(TypeTag::UnknownText),
                    payload.as_ref(),
                ),
                None => Cell::null(),
            })
            .collect::<Vec<_>>();
        let row_start = buf.len();
        let mut encoded_row = Vec::new();
        sample_encode::encode_row(&mut encoded_row, &cells)
            .with_context(|| format!("encoding sample row from {qname}"))?;
        if buf.len().saturating_add(encoded_row.len())
            > crate::engine_common::MAX_LIVE_TABLE_SAMPLE_BYTES
        {
            break;
        }
        for (col_idx, cell) in cells.iter().enumerate() {
            if let Some(accumulator) = cardinality_accumulators.get_mut(col_idx) {
                accumulator.push(cell);
            }
            if matches!(cell.tag, TypeTag::BinaryRaw | TypeTag::VectorBinary) {
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

    let source_rows = table.reltuples.max(0.0).round() as u64;
    let column_lengths = column_payload_lengths
        .into_iter()
        .map(|lengths| sampled_column_length_stats(lengths, length_fidelity))
        .collect();
    let cardinalities = cardinality_accumulators
        .iter()
        .map(|accumulator| {
            accumulator.finish(
                source_rows,
                sample_method.as_str(),
                sampled_with_bias,
                bias_reason.as_str(),
            )
        })
        .collect();
    let null_fractions = cardinality_accumulators
        .iter()
        .map(sample_encode::CardinalityAccumulator::null_fraction)
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
            sampled_with_bias,
            bias_reason,
            compression_chunk_bytes: None,
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
    }))
}

// ---------------------------------------------------------------------------
// Style classification (Tier 2 only, opt-in via consent prompt)
// ---------------------------------------------------------------------------

fn is_text_like(type_str: &str) -> bool {
    let t = type_str.to_ascii_lowercase();
    t.starts_with("text")
        || t.starts_with("character varying")
        || t.starts_with("varchar")
        || t.starts_with("character")
        || t.starts_with("char")
        || t.starts_with("jsonb")
        || t.starts_with("json")
        || t.starts_with("xml")
}

async fn peek_column_style(
    client: &tokio_postgres::Client,
    table: &TableRow,
    col: &ColumnRow,
) -> Result<&'static str> {
    let qname = format!(
        "\"{}\".\"{}\"",
        table.schema_name.replace('"', "\"\""),
        table.table_name.replace('"', "\"\"")
    );
    let qattname = format!("\"{}\"", col.attname.replace('"', "\"\""));
    let per_row_chars = (STYLE_PEEK_BYTES / 32 / 4).max(1);
    // Sample 32 rows; classify on concatenated bytes (style classifier is buffer-based).
    let sql = format!(
        "SELECT LEFT({qattname}::text, {per_row_chars}) FROM {qname} TABLESAMPLE SYSTEM (0.1) REPEATABLE (0) LIMIT 32"
    );
    let (mut rows, primary_error) = match client.query(&sql, &[]).await {
        Ok(rows) => (rows, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    if rows.is_empty() {
        // Page-level TABLESAMPLE commonly returns no page for small tables.
        // An empty successful query is therefore a sampling miss, not proof
        // that the column has no classifiable values.
        let fallback =
            format!("SELECT LEFT({qattname}::text, {per_row_chars}) FROM {qname} LIMIT 32");
        rows = client.query(&fallback, &[]).await.with_context(|| {
            if let Some(primary_error) = &primary_error {
                format!(
                    "TABLESAMPLE style probe failed ({primary_error}); fallback LIMIT probe also failed for {qname}"
                )
            } else {
                format!("fallback LIMIT style probe failed for {qname}")
            }
        })?;
    }
    let mut buf: Vec<u8> = Vec::with_capacity(STYLE_PEEK_BYTES);
    for r in rows {
        if buf.len() >= STYLE_PEEK_BYTES {
            break;
        }
        let v: Option<&str> = r
            .try_get::<_, Option<&str>>(0)
            .context("decoding PostgreSQL style sample value")?;
        if let Some(s) = v {
            buf.extend_from_slice(utf8_prefix_bytes(s, STYLE_PEEK_BYTES - buf.len()));
            buf.push(b'\n');
        }
    }
    Ok(style::classify(&buf))
}

fn utf8_prefix_bytes(value: &str, max_bytes: usize) -> &[u8] {
    let mut take = max_bytes.min(value.len());
    while !value.is_char_boundary(take) {
        take -= 1;
    }
    &value.as_bytes()[..take]
}

fn decode_pg_hex_sample(value: &str) -> Result<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.len() % 2 != 0 {
        anyhow::bail!("hex sample has an odd number of digits");
    }
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let high = pg_hex_digit(pair[0]).context("invalid high hex digit")?;
        let low = pg_hex_digit(pair[1]).context("invalid low hex digit")?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn pg_hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn pg_table_sample_percent(estimated_rows: f64, requested_rows: u64) -> f64 {
    const MIN_PERCENT: f64 = 0.1;
    const OVERSAMPLE_FACTOR: f64 = 4.0;

    if !estimated_rows.is_finite() || estimated_rows <= 0.0 || requested_rows == 0 {
        return 100.0;
    }
    (requested_rows as f64 * OVERSAMPLE_FACTOR * 100.0 / estimated_rows)
        .clamp(MIN_PERCENT, 100.0)
}

// The exact adaptive rate is only a query input: persisting it would reveal
// more precise source row estimates than the Blueprint's rounded row counts.
fn pg_sample_method(fallback_failed: bool) -> &'static str {
    if fallback_failed {
        "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (underfilled adaptive rate; LIMIT fallback failed; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
    } else {
        "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
    }
}

fn record_pg_partial_sample_warning(table_id: &str, rows: usize, audit: &mut AuditLog) {
    // Accept only the anonymous target and retained count, never the query,
    // source name, or driver error; the same redacted warning reaches both sinks.
    let detail = crate::i18n::format(
        "engine.sample_fallback_failed",
        &[
            ("code", "DBP1407W".to_string()),
            ("table", table_id.to_string()),
            ("rows", rows.to_string()),
        ],
    );
    tracing_eprintln(detail.clone());
    audit.record_warning("DBP1407W", detail);
}

#[cfg(test)]
mod sample_privacy_tests {
    use super::*;

    #[test]
    fn exported_method_does_not_embed_the_estimate_derived_rate() {
        for rows in [4_001.0, 37_777.0, 555_555.0, 3_999_999.0] {
            let rate = format!("{:.6}", pg_table_sample_percent(rows, 128));
            for failed in [false, true] {
                let method = pg_sample_method(failed);
                assert!(!method.contains(&rate));
                assert!(!method.contains("SYSTEM("));
                assert!(method.contains("TABLESAMPLE SYSTEM"));
            }
        }
    }

    #[test]
    fn partial_sample_warning_is_coded_and_retained_in_the_audit() {
        let mut audit = AuditLog::new("tier-2", 128);
        record_pg_partial_sample_warning("table-001", 17, &mut audit);
        let output = audit.render();
        assert!(output.contains("DBP1407W"));
        assert!(output.contains("table-001"));
        assert!(output.contains("17"));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
