struct PendingCompressionSample {
    ticket: CompressionTicket,
    submitted_at: Instant,
    sampled_column_indices: Vec<usize>,
    source_column_count: usize,
    column_lengths: Vec<Option<(u64, u64)>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
    payload_profiles: Vec<String>,
    complete_source_rows: Option<u64>,
}

fn mysql_column_is_transfer_value(column: &ColumnRow) -> bool {
    !matches!(
        column.value_source.as_str(),
        "generated-stored" | "generated-virtual"
    )
}

fn expand_mysql_sampled_columns<T>(
    values: Vec<Option<T>>,
    sampled_column_indices: &[usize],
    source_column_count: usize,
) -> Vec<Option<T>> {
    let mut expanded = std::iter::repeat_with(|| None)
        .take(source_column_count)
        .collect::<Vec<_>>();
    for (source_index, value) in sampled_column_indices.iter().copied().zip(values) {
        expanded[source_index] = value;
    }
    expanded
}

#[derive(Debug)]
struct MysqlPrimaryRangeSamplePlan<'a> {
    range_column: &'a ColumnRow,
    order_columns: Vec<&'a ColumnRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MysqlSampleProjectionLimit {
    byte_limit: usize,
    char_limit: usize,
}

fn mysql_is_variable_sample_column(column: &ColumnRow) -> bool {
    matches!(
        column.col_type.as_str(),
        "binary" | "text" | "json" | "user-defined"
    )
}

fn mysql_fixed_sample_payload_reserve(column: &ColumnRow) -> usize {
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
        "float" | "double" => 32,
        "date" => 10,
        "time" => 24,
        "timestamp" => 32,
        "year" => 4,
        _ => 64,
    }
}

fn mysql_sample_bytes_per_character(column: &ColumnRow) -> usize {
    if column.col_type == "binary" {
        return 1;
    }
    // The connection can transcode a narrow source charset to UTF-8. Source
    // OCTET_LENGTH is not an upper bound on the bytes returned by LEFT.
    4
}

/// Floor the prefix so a pathologically small per-column budget cannot emit
/// an always-empty LEFT(col, 0) sample. The byte limit re-covers the floored
/// prefix's worst-case transcoding so the row re-clamp stays honest.
fn mysql_floored_projection_limit(
    column: &ColumnRow,
    byte_limit: usize,
) -> MysqlSampleProjectionLimit {
    let bytes_per_character = mysql_sample_bytes_per_character(column);
    let char_limit = (byte_limit / bytes_per_character).max(1);
    MysqlSampleProjectionLimit {
        byte_limit: byte_limit.max(char_limit.saturating_mul(bytes_per_character)),
        char_limit,
    }
}

fn mysql_rows_within_sample_budget(
    planned_rows: u64,
    columns: &[ColumnRow],
    limits: &[MysqlSampleProjectionLimit],
) -> u64 {
    let payloads = columns
        .iter()
        .zip(limits)
        .map(|(column, limit)| {
            if mysql_is_variable_sample_column(column) {
                limit.byte_limit as u64
            } else {
                mysql_fixed_sample_payload_reserve(column) as u64
            }
        })
        .collect::<Vec<_>>();
    crate::engine_common::rows_within_sample_budget(planned_rows, &payloads)
}

fn mysql_sample_projection_budget(
    requested_rows: u64,
    columns: &[ColumnRow],
) -> Result<(u64, Vec<MysqlSampleProjectionLimit>)> {
    let shapes = columns
        .iter()
        .map(|column| {
            if mysql_is_variable_sample_column(column) {
                dbwarp_blueprint_core::TransferProbeColumnShape::Variable {
                    declared_max_bytes: column.char_octet_length,
                }
            } else {
                dbwarp_blueprint_core::TransferProbeColumnShape::Fixed {
                    payload_reserve: mysql_fixed_sample_payload_reserve(column) as u64,
                }
            }
        })
        .collect::<Vec<_>>();
    let plan = crate::engine_common::bounded_projection_plan(requested_rows, &shapes, false)?;
    let limits = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            if mysql_is_variable_sample_column(column) {
                let byte_limit = usize::try_from(plan.variable_byte_limits[index])
                    .unwrap_or(usize::MAX)
                    .max(1);
                mysql_floored_projection_limit(column, byte_limit)
            } else {
                MysqlSampleProjectionLimit {
                    byte_limit: 0,
                    char_limit: 0,
                }
            }
        })
        .collect::<Vec<_>>();
    let rows = mysql_rows_within_sample_budget(plan.sample_rows, columns, &limits);
    Ok((rows, limits))
}

fn mysql_sample_projection(columns: &[ColumnRow], limits: &[MysqlSampleProjectionLimit]) -> String {
    columns
        .iter()
        .zip(limits)
        .flat_map(|(column, limit)| {
            let name = quote_mysql_ident(&column.col_name);
            let sampled_value = match column.col_type.as_str() {
                "binary" => format!("LEFT({name}, {})", limit.byte_limit),
                "text" | "json" | "user-defined" => {
                    format!("LEFT({name}, {})", limit.char_limit)
                }
                _ => name.clone(),
            };
            [
                sampled_value.clone(),
                format!("OCTET_LENGTH({name})"),
                format!("OCTET_LENGTH({sampled_value})"),
            ]
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn mysql_adaptive_projection_from_observed_lengths(
    rows: &[mysql_async::Row],
    columns: &[ColumnRow],
    requested_rows: u64,
) -> Result<Option<(u64, Vec<MysqlSampleProjectionLimit>)>> {
    let (observed_max, truncated_columns) = mysql_sample_observed_lengths(rows, columns);
    if !truncated_columns.iter().any(|truncated| *truncated) {
        return Ok(None);
    }
    mysql_adaptive_projection_budget(columns, &observed_max, requested_rows).map(Some)
}

fn mysql_sample_observed_lengths(
    rows: &[mysql_async::Row],
    columns: &[ColumnRow],
) -> (Vec<u64>, Vec<bool>) {
    let mut observed_max = vec![0_u64; columns.len()];
    let mut truncated = vec![false; columns.len()];
    for row in rows {
        for (column_index, column) in columns.iter().enumerate() {
            if !mysql_is_variable_sample_column(column) {
                continue;
            }
            let value_index = column_index.saturating_mul(3);
            // Compare both lengths on the server, before result-charset
            // conversion. A one-byte latin1 prefix can arrive as two UTF-8
            // bytes and otherwise conceal truncation of a two-byte source.
            let sampled_bytes = row
                .as_ref(value_index.saturating_add(2))
                .and_then(mysql_observed_octet_length)
                .unwrap_or(0);
            let original_bytes = row
                .as_ref(value_index.saturating_add(1))
                .and_then(mysql_observed_octet_length)
                .unwrap_or(sampled_bytes);
            observed_max[column_index] = observed_max[column_index].max(original_bytes);
            truncated[column_index] |= sampled_bytes < original_bytes;
        }
    }
    (observed_max, truncated)
}

fn mysql_adaptive_projection_budget(
    columns: &[ColumnRow],
    observed_max: &[u64],
    requested_rows: u64,
) -> Result<(u64, Vec<MysqlSampleProjectionLimit>)> {
    let shapes = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            if mysql_is_variable_sample_column(column) {
                dbwarp_blueprint_core::TransferProbeColumnShape::Variable {
                    declared_max_bytes: observed_max[index]
                        .max(1)
                        .saturating_mul(mysql_sample_bytes_per_character(column) as u64),
                }
            } else {
                dbwarp_blueprint_core::TransferProbeColumnShape::Fixed {
                    payload_reserve: mysql_fixed_sample_payload_reserve(column) as u64,
                }
            }
        })
        .collect::<Vec<_>>();
    let plan = crate::engine_common::bounded_projection_plan(requested_rows, &shapes, true)?;
    let limits = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            if mysql_is_variable_sample_column(column) {
                // Same floors as the initial projection: never emit an
                // always-empty LEFT(col, 0) prefix on the retry either.
                let byte_limit = (plan.variable_byte_limits[index] as usize).max(1);
                mysql_floored_projection_limit(column, byte_limit)
            } else {
                MysqlSampleProjectionLimit {
                    byte_limit: 0,
                    char_limit: 0,
                }
            }
        })
        .collect::<Vec<_>>();
    let rows = mysql_rows_within_sample_budget(plan.sample_rows, columns, &limits);
    Ok((rows, limits))
}

fn mysql_observed_octet_length(value: &Value) -> Option<u64> {
    match value {
        Value::NULL => None,
        Value::Bytes(bytes) => std::str::from_utf8(bytes).ok()?.parse().ok(),
        Value::Int(value) => u64::try_from(*value).ok(),
        Value::UInt(value) => Some(*value),
        Value::Float(value) => (*value >= 0.0).then_some(*value as u64),
        Value::Double(value) => (*value >= 0.0).then_some(*value as u64),
        Value::Date(..) | Value::Time(..) => None,
    }
}

async fn sample_compression(
    conn: &mut mysql_async::Conn,
    t: &TableRow,
    table_columns: &[ColumnRow],
    range_sample_plan: Option<&MysqlPrimaryRangeSamplePlan<'_>>,
    sample_rows: u64,
    length_fidelity: LengthFidelity,
    compression_pool: &CompressionWorkerPool,
    audit: &mut AuditLog,
) -> Result<Option<PendingCompressionSample>> {
    if table_columns.is_empty() {
        return Ok(None);
    }
    let sampled_column_indices = table_columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| mysql_column_is_transfer_value(column).then_some(index))
        .collect::<Vec<_>>();
    if sampled_column_indices.is_empty() {
        return Ok(None);
    }
    let sampled_columns = sampled_column_indices
        .iter()
        .map(|index| table_columns[*index].clone())
        .collect::<Vec<_>>();
    let qname = format!(
        "`{}`.`{}`",
        t.schema_name.replace('`', "``"),
        t.table_name.replace('`', "``")
    );
    let (bounded_rows, projection_limits) =
        mysql_sample_projection_budget(sample_rows, sampled_columns.as_slice())?;
    let projection = mysql_sample_projection(sampled_columns.as_slice(), &projection_limits);
    let (mut rows, first_method, first_bias, first_layout, first_complete_row_read) =
        query_mysql_compression_sample(
            conn,
            t,
            &qname,
            &projection,
            range_sample_plan,
            bounded_rows,
            audit,
        )
        .await?;
    let first = MysqlRetainedSampleProvenance {
        method: first_method.to_string(),
        bias_reason: first_bias.to_string(),
        layout: first_layout,
        complete_row_read: first_complete_row_read,
    };
    let mut retry = None;
    if let Some((retry_rows, retry_limits)) = mysql_adaptive_projection_from_observed_lengths(
        rows.as_slice(),
        sampled_columns.as_slice(),
        bounded_rows,
    )? {
        let retry_projection = mysql_sample_projection(sampled_columns.as_slice(), &retry_limits);
        drop(rows);
        let (retry_result, retry_method, retry_bias, retry_layout, retry_complete_row_read) =
            query_mysql_compression_sample(
                conn,
                t,
                &qname,
                &retry_projection,
                range_sample_plan,
                retry_rows,
                audit,
            )
            .await?;
        rows = retry_result;
        retry = Some(MysqlRetainedSampleProvenance {
            method: retry_method.to_string(),
            bias_reason: retry_bias.to_string(),
            layout: retry_layout,
            complete_row_read: retry_complete_row_read,
        });
    }
    let MysqlRetainedSampleProvenance {
        method: mut sample_method,
        mut bias_reason,
        layout: sample_layout,
        mut complete_row_read,
    } = mysql_retained_sample_provenance(first, retry);
    let (_, column_value_truncated) =
        mysql_sample_observed_lengths(&rows, &sampled_columns);
    if column_value_truncated.iter().any(|truncated| *truncated) {
        crate::engine_common::record_sample_prefix_bias(&mut sample_method, &mut bias_reason);
    }
    if rows.is_empty() {
        return Ok(None);
    }

    // Encode rows using the transient compression-probe representation. Each cell is
    // tagged via `encode_mysql_cell` based on column metadata + the
    // value variant. The driver returns most values as `Value::Bytes`
    // (textual for numerics; UTF-8 / charset bytes for text; raw bytes for
    // BLOB-with-binary-charset). Tagged, length-prefixed cells keep binary and complex
    // values distinct from genuinely empty fields.
    let projected_columns = rows[0].columns_ref();
    let internal_range_columns = usize::from(matches!(
        sample_layout,
        format::BlueprintSampleLayout::PrimaryKeyRangeWindows
    ));
    if projected_columns.len()
        != sampled_columns
            .len()
            .saturating_mul(3)
            .saturating_add(internal_range_columns)
    {
        bail!(
            "MySQL compression sample returned {} fields for {} source columns",
            projected_columns.len(),
            sampled_columns.len()
        );
    }
    let column_metadata: Vec<(MyColumnType, u16)> = projected_columns
        .iter()
        .step_by(3)
        .map(|c| (c.column_type(), c.character_set()))
        .collect::<Vec<_>>();

    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut row_ranges: Vec<(usize, usize)> = Vec::with_capacity(rows.len());
    let mut column_bufs: Vec<Vec<u8>> = Vec::new();
    let mut column_payload_lengths: Vec<Vec<u64>> = Vec::new();
    let mut cardinality_accumulators: Vec<sample_encode::CardinalityAccumulator> = Vec::new();
    let mut payload_profile_accumulators: Vec<dbwarp_blueprint_core::PayloadProfileAccumulator> =
        Vec::new();

    for r in &rows {
        let n_cols = sampled_columns.len();
        if column_bufs.is_empty() {
            column_bufs = vec![Vec::new(); n_cols];
            column_payload_lengths = vec![Vec::new(); n_cols];
            cardinality_accumulators =
                vec![sample_encode::CardinalityAccumulator::default(); n_cols];
            payload_profile_accumulators =
                vec![dbwarp_blueprint_core::PayloadProfileAccumulator::default(); n_cols];
        }
        // Build owned (TypeTag, Vec<u8>) per cell, then borrow for the encoder.
        let cells_owned: Vec<(TypeTag, Vec<u8>)> = (0..n_cols)
            .map(|idx| {
                let v_ref = r.as_ref(idx.saturating_mul(3)).unwrap_or(&Value::NULL);
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
        for (col_idx, (tag, _)) in cells_owned.iter().enumerate() {
            if *tag != TypeTag::Null {
                if let Some(length) = r
                    .as_ref(col_idx.saturating_mul(3).saturating_add(1))
                    .and_then(mysql_observed_octet_length)
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
        .map(|lengths| sampled_column_length_stats(lengths, length_fidelity))
        .collect();
    let cardinalities = cardinality_accumulators
        .iter()
        .enumerate()
        .map(|(index, accumulator)| {
            accumulator
                .finish_with_source_rows(
                    complete_row_read
                        .then_some(row_ranges.len() as u64)
                        .or(t.rows_estimate),
                    complete_row_read,
                    complete_row_read && !column_value_truncated[index],
                    sample_method.as_str(),
                    true,
                    bias_reason.as_str(),
                )
                .map(|mut cardinality| {
                    cardinality.sample_layout = sample_layout;
                    cardinality
                })
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
        sampled_column_indices,
        source_column_count: table_columns.len(),
        column_lengths,
        null_fractions,
        cardinalities,
        payload_profiles,
        complete_source_rows: complete_row_read.then_some(encoded_sample_rows),
    }))
}

const MYSQL_COMPRESSION_RANGE_WINDOWS: u64 = 4;

fn mysql_bounded_result_is_complete(returned_rows: usize, requested_rows: u64) -> bool {
    returned_rows < requested_rows as usize
}

fn mysql_primary_range_sample_plan<'a>(
    qual: &(String, String),
    columns: &'a [ColumnRow],
    indexes: &BTreeMap<(String, String, String), Vec<IndexPart>>,
) -> Option<MysqlPrimaryRangeSamplePlan<'a>> {
    let mut primary_parts = indexes
        .iter()
        .filter(|((schema, table, _), _)| schema == &qual.0 && table == &qual.1)
        .flat_map(|(_, parts)| parts.iter())
        .filter(|part| part.4 && !part.7 && !part.1.is_empty())
        .collect::<Vec<_>>();
    primary_parts.sort_by_key(|part| part.0);
    let first_primary_name = primary_parts.first()?.1.as_str();
    let range_column = columns.iter().find(|column| {
        column.col_name.eq_ignore_ascii_case(first_primary_name)
            && column.numeric_scale == 0
            && matches!(
                column.native_type.as_str(),
                "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint"
            )
    })?;
    let order_columns = primary_parts
        .into_iter()
        .map(|part| {
            columns
                .iter()
                .find(|column| column.col_name.eq_ignore_ascii_case(part.1.as_str()))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(MysqlPrimaryRangeSamplePlan {
        range_column,
        order_columns,
    })
}

async fn query_mysql_compression_sample(
    conn: &mut mysql_async::Conn,
    table: &TableRow,
    qname: &str,
    projection: &str,
    range_sample_plan: Option<&MysqlPrimaryRangeSamplePlan<'_>>,
    bounded_rows: u64,
    audit: &mut AuditLog,
) -> Result<(
    Vec<mysql_async::Row>,
    &'static str,
    &'static str,
    format::BlueprintSampleLayout,
    bool,
)> {
    if let Some(plan) = range_sample_plan.filter(|_| {
        table.rows_estimate.unwrap_or(0)
            >= bounded_rows
                .saturating_mul(MYSQL_COMPRESSION_RANGE_WINDOWS)
                .max(1)
    }) {
        let column_name = quote_mysql_ident(&plan.range_column.col_name);
        let order_columns = plan
            .order_columns
            .iter()
            .map(|column| quote_mysql_ident(&column.col_name))
            .collect::<Vec<_>>()
            .join(", ");
        let bounds_sql = format!(
            "SELECT CAST(MIN({column_name}) AS CHAR), CAST(MAX({column_name}) AS CHAR) FROM {qname}"
        );
        let started = Instant::now();
        let bounds = conn
            .query_first::<(Option<String>, Option<String>), _>(bounds_sql)
            .await;
        match bounds {
            Ok(Some((Some(minimum), Some(maximum)))) => {
                audit.record_query(
                    "SELECT MIN/MAX numeric primary-key bounds FROM <table> (compression range planning; values discarded)",
                    elapsed_ms(started),
                    1,
                );
                if let (Ok(minimum), Ok(maximum)) =
                    (minimum.parse::<i128>(), maximum.parse::<i128>())
                {
                    if maximum >= minimum {
                        let windows = mysql_range_sample_windows(
                            minimum,
                            maximum,
                            MYSQL_COMPRESSION_RANGE_WINDOWS,
                        );
                        let window_count = u64::try_from(windows.len()).unwrap_or(1).max(1);
                        let base_rows = bounded_rows / window_count;
                        let extra_rows = bounded_rows % window_count;
                        let mut window_limits = Vec::with_capacity(windows.len());
                        let mut window_queries = Vec::with_capacity(windows.len());
                        let range_started = Instant::now();
                        for (window, (lower_bound, upper_bound)) in
                            windows.into_iter().enumerate()
                        {
                            let window = window as u64;
                            let window_rows = base_rows + u64::from(window < extra_rows);
                            if window_rows == 0 {
                                continue;
                            }
                            let upper_predicate = upper_bound
                                .map(|upper| format!(" AND {column_name} < {upper}"))
                                .unwrap_or_default();
                            window_limits.push(window_rows);
                            // Parenthesized query expressions keep each
                            // window's ORDER BY/LIMIT local while UNION ALL
                            // makes every range read part of one statement and
                            // therefore one InnoDB consistent-read snapshot.
                            window_queries.push(format!(
                                "(SELECT {projection}, {window} AS __dbwarp_blueprint_window FROM {qname} FORCE INDEX (PRIMARY) WHERE {column_name} >= {lower_bound}{upper_predicate} ORDER BY {order_columns} LIMIT {window_rows})"
                            ));
                        }
                        if !window_queries.is_empty() {
                            let sql = window_queries.join(" UNION ALL ");
                            match conn.query::<mysql_async::Row, _>(sql).await {
                                Ok(sampled) => {
                                    let window_counts = mysql_range_window_counts(
                                        sampled.as_slice(),
                                        window_limits.len(),
                                    );
                                    if window_counts.is_some() {
                                        audit.record_query(
                                            "one SELECT UNION ALL containing type-budgeted projections plus original OCTET_LENGTH from up to four non-overlapping numeric primary-key ranges (compression sample)",
                                            elapsed_ms(range_started),
                                            sampled.len() as u64,
                                        );
                                        if !sampled.is_empty() {
                                            return Ok((
                                                sampled,
                                                "single-statement up-to-four-window numeric primary-key range sample (MySQL; type-budgeted projection; original octet lengths observed separately; non-final windows have exclusive upper bounds)",
                                                "numeric_primary_key_range_windows+single_statement_snapshot+no_native_tablesample+type_budgeted_projection+non_overlapping_upper_bounds",
                                                format::BlueprintSampleLayout::PrimaryKeyRangeWindows,
                                                // MIN/MAX came from a prior statement. Even if
                                                // every window is underfilled, concurrent changes
                                                // outside those remembered bounds mean this is not
                                                // proof that the visible table was read completely.
                                                mysql_sample_layout_can_prove_complete_read(
                                                    format::BlueprintSampleLayout::PrimaryKeyRangeWindows,
                                                ),
                                            ));
                                        }
                                    } else {
                                        audit.record_query_failure(
                                            "one SELECT UNION ALL containing type-budgeted projections plus original OCTET_LENGTH from up to four non-overlapping numeric primary-key ranges (compression sample; invalid internal window ordinal)",
                                            elapsed_ms(range_started),
                                        );
                                    }
                                }
                                Err(_) => audit.record_query_failure(
                                    "one SELECT UNION ALL containing type-budgeted projections plus original OCTET_LENGTH from up to four non-overlapping numeric primary-key ranges (compression sample)",
                                    elapsed_ms(range_started),
                                ),
                            }
                        }
                    }
                }
            }
            Ok(_) => audit.record_query(
                "SELECT MIN/MAX numeric primary-key bounds FROM <table> (compression range planning; values discarded)",
                elapsed_ms(started),
                0,
            ),
            Err(_) => audit.record_query_failure(
                "SELECT MIN/MAX numeric primary-key bounds FROM <table> (compression range planning; values discarded)",
                elapsed_ms(started),
            ),
        }
    }

    let sql = format!("SELECT {projection} FROM {qname} LIMIT {bounded_rows}");
    let started = Instant::now();
    let rows: Vec<mysql_async::Row> = conn
        .query(sql)
        .await
        .with_context(|| format!("sampling {qname}"))?;
    audit.record_query(
        "SELECT type-budgeted bounded projection plus original OCTET_LENGTH FROM <table> LIMIT N (compression sample)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let complete_row_read = mysql_bounded_result_is_complete(rows.len(), bounded_rows);
    Ok((
        rows,
        "LIMIT N (MySQL; type-budgeted projection; original octet lengths observed separately)",
        "natural_pk_order_no_native_tablesample+type_budgeted_projection",
        format::BlueprintSampleLayout::Unknown,
        complete_row_read,
    ))
}

/// Provenance of the rows a MySQL compression sample actually retained.
#[derive(Debug, Clone, PartialEq)]
struct MysqlRetainedSampleProvenance {
    method: String,
    bias_reason: String,
    layout: format::BlueprintSampleLayout,
    complete_row_read: bool,
}

/// When the adaptive byte-bounded retry runs, its rows replace the first
/// query's, so every provenance field must come from the retry. The retry can
/// fall back from primary-key range windows to `LIMIT` when its own range
/// statement fails, and the serialized layout must describe the kept rows
/// rather than the discarded first attempt.
fn mysql_retained_sample_provenance(
    first: MysqlRetainedSampleProvenance,
    retry: Option<MysqlRetainedSampleProvenance>,
) -> MysqlRetainedSampleProvenance {
    match retry {
        None => first,
        Some(retry) => MysqlRetainedSampleProvenance {
            method: format!(
                "{}; adaptive byte-bounded retry from observed octet lengths",
                retry.method
            ),
            bias_reason: format!("{}+adaptive_observed_octet_length_retry", retry.bias_reason),
            layout: retry.layout,
            complete_row_read: retry.complete_row_read,
        },
    }
}

fn mysql_sample_layout_can_prove_complete_read(
    layout: format::BlueprintSampleLayout,
) -> bool {
    !matches!(layout, format::BlueprintSampleLayout::PrimaryKeyRangeWindows)
}

fn mysql_range_sample_thresholds(minimum: i128, maximum: i128, windows: u64) -> Vec<i128> {
    let windows = windows.max(1);
    let span = maximum.saturating_sub(minimum);
    (0..windows)
        .map(|window| {
            minimum.saturating_add(span.saturating_mul(i128::from(window)) / i128::from(windows))
        })
        .collect()
}

fn mysql_range_sample_windows(
    minimum: i128,
    maximum: i128,
    windows: u64,
) -> Vec<(i128, Option<i128>)> {
    let mut thresholds = mysql_range_sample_thresholds(minimum, maximum, windows);
    thresholds.dedup();
    thresholds
        .iter()
        .copied()
        .enumerate()
        .map(|(index, lower)| (lower, thresholds.get(index + 1).copied()))
        .collect()
}

fn mysql_range_window_counts(rows: &[mysql_async::Row], windows: usize) -> Option<Vec<u64>> {
    let mut counts = vec![0_u64; windows];
    for row in rows {
        let ordinal_index = row.columns_ref().len().checked_sub(1)?;
        let ordinal = row
            .as_ref(ordinal_index)
            .and_then(mysql_observed_octet_length)
            .and_then(|ordinal| usize::try_from(ordinal).ok())?;
        let count = counts.get_mut(ordinal)?;
        *count = count.saturating_add(1);
    }
    Some(counts)
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
    let sql = format!("SELECT LEFT(CAST({qcol} AS CHAR), {per_row_chars}) FROM {qname} LIMIT 32");
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
