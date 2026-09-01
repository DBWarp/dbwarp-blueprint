struct PendingCompressionSample {
    ticket: CompressionTicket,
    submitted_at: Instant,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
}

async fn sample_compression(
    client: &tokio_postgres::Client,
    table: &TableRow,
    table_columns: &[ColumnRow],
    sample_rows: u64,
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
    let (bounded_rows, cell_char_limit) = crate::engine_common::live_sample_budget(
        sample_rows,
        table_columns.len(),
        4,
    );
    let projection = table_columns
        .iter()
        .map(|column| {
            let name = format!("\"{}\"", column.attname.replace('"', "\"\""));
            format!("LEFT({name}::text, {cell_char_limit})")
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

    // Use TABLESAMPLE SYSTEM(0.1) plus LIMIT to cap at sample_rows.
    // This is page-level sampling — fast, statistically representative.
    //
    // We use the *simple query* protocol (not the extended `client.query`
    // path) so the server returns column values in TEXT format. The
    // extended-query path opportunistically uses BINARY format for
    // types whose `FromSql` impl accepts it — and the byte distribution
    // of binary-format wire traffic is meaningfully different from the
    // text-format traffic that most ORM-driven applications (psycopg,
    // pgjdbc default, Go pgx default-ish) actually move. Sampling in
    // text format is therefore the more representative choice for
    // estimating real-world transfer compression.
    let sql = format!(
        "SELECT {projection} FROM {qname} TABLESAMPLE SYSTEM (0.1) REPEATABLE (0) LIMIT {bounded_rows}"
    );
    let started = Instant::now();
    let mut sampled_with_bias = true;
    let mut bias_reason = "server_side_cell_cap".to_string();
    let mut sample_method =
        "TABLESAMPLE SYSTEM(0.1) REPEATABLE(0) LIMIT N (text format; server-side cell cap)"
            .to_string();
    let mut primary_error = None;
    let mut messages = match client.simple_query(&sql).await {
        Ok(m) => m,
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
    if rows.is_empty() {
        let fallback = format!("SELECT {projection} FROM {qname} LIMIT {bounded_rows}");
        let mut fb = client.simple_query(&fallback).await.with_context(|| {
            if let Some(primary_error) = &primary_error {
                format!(
                    "TABLESAMPLE query failed ({primary_error}); fallback LIMIT query also failed for {qname}"
                )
            } else {
                format!("fallback LIMIT query failed for {qname}")
            }
        })?;
        rows = fb
            .drain(..)
            .filter_map(|m| match m {
                SimpleQueryMessage::Row(r) => Some(r),
                _ => None,
            })
            .collect();
        sampled_with_bias = true;
        bias_reason = "unordered_limit_after_empty_TABLESAMPLE+server_side_cell_cap".to_string();
        sample_method =
            "LIMIT N (fallback after empty TABLESAMPLE; text format; server-side cell cap)"
                .to_string();
    }
    if rows.is_empty() {
        return Ok(None);
    }
    audit.record_query(
        &format!(
            "{} on a single user table (compression sample)",
            sample_method
        ),
        elapsed_ms(started),
        rows.len() as u64,
    );

    // Encode rows using `dbwarp-blueprint-rowframe-v1`. Each column carries
    // its TEXT-format wire bytes (UTF-8 for everything tokio-postgres
    // text-mode returns) plus a type tag from the catalog scan. The
    // resulting buffer's byte distribution closely tracks default
    // text-format wire traffic. The tagged, length-prefixed framing prevents
    // non-text columns from collapsing into ambiguous empty fields. The full
    // encoding contract and regression tests live in `src/sample_encode.rs`.
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut row_ranges: Vec<(usize, usize)> = Vec::with_capacity(rows.len());
    let mut column_bufs: Vec<Vec<u8>> = Vec::new();
    let mut cardinality_accumulators: Vec<sample_encode::CardinalityAccumulator> = Vec::new();

    for r in &rows {
        let n_cols = r.columns().len();
        if column_bufs.is_empty() {
            column_bufs = vec![Vec::new(); n_cols];
            cardinality_accumulators =
                vec![sample_encode::CardinalityAccumulator::default(); n_cols];
        }
        let mut cells: Vec<Cell<'_>> = Vec::with_capacity(n_cols);
        for col_idx in 0..n_cols {
            let cell_text: Option<&str> = r.get(col_idx);
            match cell_text {
                Some(s) => {
                    let tag = column_tags
                        .get(col_idx)
                        .copied()
                        .unwrap_or(TypeTag::UnknownText);
                    cells.push(Cell::new(tag, s.as_bytes()));
                }
                None => cells.push(Cell::null()),
            }
        }
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

    let encoded_sample_rows = row_ranges.len() as u64;
    let submitted_at = Instant::now();
    let ticket = compression_pool
        .submit(PreparedCompressionSample {
            table_bytes: buf,
            row_ranges,
            column_bytes: column_bufs,
            sample_rows: encoded_sample_rows,
            sample_method,
            sampled_with_bias,
            bias_reason,
        })
        .with_context(|| format!("submitting local compression work for {qname}"))?;
    audit.record_compression_job_submitted();

    Ok(Some(PendingCompressionSample {
        ticket,
        submitted_at,
        null_fractions,
        cardinalities,
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
