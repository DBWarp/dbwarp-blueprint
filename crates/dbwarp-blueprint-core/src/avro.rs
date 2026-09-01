use crate::{
    BlueprintColumn, BlueprintCompression, BlueprintFile, BlueprintTable, SamplingDeadline, Totals,
    SCHEMA_VERSION,
};
use anyhow::{Context, Result};
use apache_avro::{types::Value, Reader, Schema};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[cfg(feature = "sampling")]
use crate::{CompressionSampleAccumulator, DecodedCompressionOptions, OwnedCell, TypeTag};

const STRUCTURED_TABLE_ID: &str = "table-001";
const DEFAULT_AVRO_SCAN_MEMORY_BYTES: usize = 256 * 1024 * 1024;
const AVRO_MAGIC: &[u8; 4] = b"Obj\x01";
const MAX_AVRO_HEADER_BYTES: usize = 8 * 1024 * 1024;
const MAX_AVRO_SCHEMA_BYTES: usize = 4 * 1024 * 1024;
const MAX_AVRO_METADATA_ENTRIES: usize = 128;
const MAX_AVRO_METADATA_KEY_BYTES: usize = 256;
const MAX_AVRO_SCHEMA_DEPTH: usize = 64;

struct BoundedAvroHeader<'a, R> {
    reader: &'a mut R,
    bytes_read: usize,
}

impl<'a, R: Read> BoundedAvroHeader<'a, R> {
    fn new(reader: &'a mut R) -> Self {
        Self {
            reader,
            bytes_read: 0,
        }
    }

    fn read_exact(&mut self, output: &mut [u8]) -> Result<()> {
        let next = self
            .bytes_read
            .checked_add(output.len())
            .context("Avro header byte count overflowed")?;
        if next > MAX_AVRO_HEADER_BYTES {
            anyhow::bail!("Avro header exceeds the {MAX_AVRO_HEADER_BYTES}-byte safety limit");
        }
        self.reader
            .read_exact(output)
            .context("reading bounded Avro object-container header")?;
        self.bytes_read = next;
        Ok(())
    }

    fn read_long(&mut self) -> Result<i64> {
        let mut encoded = 0u64;
        for index in 0..10 {
            let mut byte = [0u8; 1];
            self.read_exact(&mut byte)?;
            if index == 9 && (byte[0] & 0xfe) != 0 {
                anyhow::bail!("Avro header contains an out-of-range long");
            }
            encoded |= u64::from(byte[0] & 0x7f) << (index * 7);
            if byte[0] & 0x80 == 0 {
                return Ok(((encoded >> 1) as i64) ^ (-((encoded & 1) as i64)));
            }
        }
        anyhow::bail!("Avro header contains an unterminated long")
    }

    fn read_length(&mut self, label: &str, maximum: usize) -> Result<usize> {
        let length = self.read_long()?;
        let length = usize::try_from(length)
            .with_context(|| format!("Avro {label} length is negative or unsupported"))?;
        if length > maximum {
            anyhow::bail!("Avro {label} exceeds its {maximum}-byte safety limit");
        }
        Ok(length)
    }

    fn skip(&mut self, mut length: usize) -> Result<()> {
        let mut buffer = [0u8; 4096];
        while length > 0 {
            let chunk = length.min(buffer.len());
            self.read_exact(&mut buffer[..chunk])?;
            length -= chunk;
        }
        Ok(())
    }
}

fn open_preflighted_avro(path: &Path, purpose: &str) -> Result<File> {
    let mut file = File::open(path)
        .with_context(|| format!("opening Avro file {} {purpose}", path.display()))?;
    preflight_avro_container(&mut file)
        .with_context(|| format!("checking Avro schema safety in {}", path.display()))?;
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("rewinding Avro file {} {purpose}", path.display()))?;
    Ok(file)
}

fn preflight_avro_container<R: Read>(reader: &mut R) -> Result<()> {
    let mut header = BoundedAvroHeader::new(reader);
    let mut magic = [0u8; 4];
    header.read_exact(&mut magic)?;
    if &magic != AVRO_MAGIC {
        anyhow::bail!("Avro object-container magic is invalid");
    }

    let mut schema = None;
    let mut metadata_entries = 0usize;
    loop {
        let encoded_count = header.read_long()?;
        if encoded_count == 0 {
            break;
        }
        let (entry_count, declared_block_bytes) = if encoded_count < 0 {
            let count = encoded_count
                .checked_neg()
                .context("Avro metadata block count is out of range")?;
            let block_bytes = header.read_long()?;
            let block_bytes = usize::try_from(block_bytes)
                .context("Avro metadata block byte length is negative or unsupported")?;
            (count, Some(block_bytes))
        } else {
            (encoded_count, None)
        };
        let entry_count =
            usize::try_from(entry_count).context("Avro metadata entry count is unsupported")?;
        metadata_entries = metadata_entries
            .checked_add(entry_count)
            .context("Avro metadata entry count overflowed")?;
        if metadata_entries > MAX_AVRO_METADATA_ENTRIES {
            anyhow::bail!(
                "Avro header exceeds the {MAX_AVRO_METADATA_ENTRIES}-entry metadata safety limit"
            );
        }
        let block_start = header.bytes_read;
        for _ in 0..entry_count {
            let key_length = header.read_length("metadata key", MAX_AVRO_METADATA_KEY_BYTES)?;
            let mut key = vec![0u8; key_length];
            header.read_exact(&mut key)?;
            let value_length = header.read_length("metadata value", MAX_AVRO_HEADER_BYTES)?;
            if key.as_slice() == b"avro.schema" {
                if value_length > MAX_AVRO_SCHEMA_BYTES {
                    anyhow::bail!(
                        "Avro schema exceeds the {MAX_AVRO_SCHEMA_BYTES}-byte safety limit"
                    );
                }
                let mut bytes = vec![0u8; value_length];
                header.read_exact(&mut bytes)?;
                schema = Some(bytes);
            } else {
                header.skip(value_length)?;
            }
        }
        if let Some(declared) = declared_block_bytes {
            let consumed = header
                .bytes_read
                .checked_sub(block_start)
                .context("Avro metadata block accounting underflowed")?;
            if consumed != declared {
                anyhow::bail!("Avro metadata block byte length is inconsistent");
            }
        }
    }

    let mut sync_marker = [0u8; 16];
    header.read_exact(&mut sync_marker)?;
    let schema = schema.context("Avro header does not contain avro.schema metadata")?;
    validate_avro_schema_nesting(&schema)
}

fn validate_avro_schema_nesting(schema: &[u8]) -> Result<()> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in schema {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .context("Avro schema nesting counter overflowed")?;
                if depth > MAX_AVRO_SCHEMA_DEPTH {
                    anyhow::bail!(
                        "Avro schema nesting exceeds the {MAX_AVRO_SCHEMA_DEPTH}-level safety limit"
                    );
                }
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .context("Avro schema contains an unmatched closing delimiter")?;
            }
            _ => {}
        }
    }
    if in_string || depth != 0 {
        anyhow::bail!("Avro schema JSON is structurally incomplete");
    }
    Ok(())
}

/// Build a DBWarp Blueprint model from an Avro object container file.
///
/// Avro containers do not expose a footer-level row count like Parquet, so this
/// walks the file to count records while deriving column blueprint from the writer
/// schema.
pub fn avro_blueprint_from_path(path: impl AsRef<Path>) -> Result<BlueprintFile> {
    avro_blueprint_from_path_with_deadline(path, &SamplingDeadline::unlimited())
}

/// Build an Avro Blueprint while honoring an operation-wide deadline supplied by
/// the caller. Reuse the same deadline for every file in a batch so the
/// wall-clock budget is not restarted at file boundaries.
pub fn avro_blueprint_from_path_with_deadline(
    path: impl AsRef<Path>,
    deadline: &SamplingDeadline,
) -> Result<BlueprintFile> {
    avro_blueprint_from_path_metadata(path, DEFAULT_AVRO_SCAN_MEMORY_BYTES, deadline)
}

#[cfg(feature = "sampling")]
pub fn avro_blueprint_from_path_with_options(
    path: impl AsRef<Path>,
    options: &DecodedCompressionOptions,
) -> Result<BlueprintFile> {
    let deadline = options.deadline();
    avro_blueprint_from_path_with_options_and_deadline(path, options, &deadline)
}

/// Build and sample an Avro Blueprint under a caller-owned, absolute deadline.
/// The deadline covers the full row scan, decoded sampling, and compression.
#[cfg(feature = "sampling")]
pub fn avro_blueprint_from_path_with_options_and_deadline(
    path: impl AsRef<Path>,
    options: &DecodedCompressionOptions,
    deadline: &SamplingDeadline,
) -> Result<BlueprintFile> {
    let path = path.as_ref();
    let mut blueprint =
        avro_blueprint_from_path_metadata(path, options.max_sample_bytes, deadline)?;
    if options.is_enabled() {
        apply_avro_decoded_compression(path, &mut blueprint, options, deadline)?;
    }
    Ok(blueprint)
}

fn avro_blueprint_from_path_metadata(
    path: impl AsRef<Path>,
    max_resident_bytes: usize,
    deadline: &SamplingDeadline,
) -> Result<BlueprintFile> {
    let path = path.as_ref();
    deadline.check("opening Avro metadata")?;
    let file = open_preflighted_avro(path, "for metadata capture")?;
    let mut reader = Reader::new(file)
        .with_context(|| format!("reading Avro header from {}", path.display()))?;
    let schema = reader.writer_schema().clone();
    let mut table = BlueprintTable {
        storage_bytes: std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0),
        schema: "avro".to_string(),
        source_partitions: 1,
        // apache-avro 0.21 parses the codec internally but exposes neither
        // the parsed codec nor reserved avro.* header metadata on Reader.
        source_codec: String::new(),
        ..Default::default()
    };
    fill_avro_columns(&schema, &mut table);
    let ordered_column_ids = crate::ordered_columns(&table)
        .into_iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let width_capacity = width_reservoir_capacity(ordered_column_ids.len(), max_resident_bytes)?;
    let mut observed = (0..ordered_column_ids.len())
        .map(|_| ObservedColumnStats::new(width_capacity))
        .collect::<Vec<_>>();
    let observed_columns = ordered_column_ids
        .iter()
        .filter_map(|name| table.cols.get(name).cloned())
        .collect::<Vec<_>>();
    let mut row_count = 0u64;
    let mut logical_bytes = 0u64;
    for value_result in reader.by_ref() {
        deadline.check("scanning Avro Blueprint rows")?;
        let value = value_result.context("decoding Avro row for Blueprint statistics")?;
        logical_bytes = logical_bytes.saturating_add(observe_avro_row(
            &value,
            &mut observed,
            &observed_columns,
        ));
        row_count = row_count.saturating_add(1);
    }
    table.rows = row_count;
    table.table_bytes = logical_bytes;
    for (name, statistics) in ordered_column_ids.iter().zip(observed.iter()) {
        deadline.check("computing Avro width statistics")?;
        if let Some(column) = table.cols.get_mut(name) {
            column.null_fraction = Some(statistics.null_fraction());
            column.len_avg = statistics.len_avg();
            column.len_p95 = statistics.len_p95();
            column.length_sample_rows = statistics.rows;
            column.length_p95_sample_rows = statistics.lengths.len() as u64;
            column.length_sample_method =
                "avro-decoded-full-scan-bounded-p95-reservoir".to_string();
        }
    }
    table.compression = Some(BlueprintCompression {
        measured: table.storage_bytes > 0 || table.table_bytes > 0,
        sample_rows: row_count,
        sample_bytes: table.table_bytes,
        sample_method: "avro-container-versus-decoded-rowframe".to_string(),
        ratio_storage: compression_ratio(table.table_bytes, table.storage_bytes),
        sample_encoding: "avro-container".to_string(),
        ..Default::default()
    });

    deadline.check("finishing Avro metadata")?;
    let mut tables = BTreeMap::new();
    tables.insert(STRUCTURED_TABLE_ID.to_string(), table);
    Ok(BlueprintFile {
        schema_version: SCHEMA_VERSION,
        engine: "avro".to_string(),
        source_kind: "avro".to_string(),
        totals: Totals {
            table_count: 1,
            row_count,
            table_bytes: logical_bytes,
            index_bytes: 0,
        },
        dataset_scope: Some(crate::DatasetScope::structured_dataset(
            "avro-decoded-scan",
            "avro-container",
        )),
        tables,
        ..Default::default()
    })
}

#[cfg(feature = "sampling")]
fn apply_avro_decoded_compression(
    path: &Path,
    blueprint: &mut BlueprintFile,
    options: &DecodedCompressionOptions,
    deadline: &SamplingDeadline,
) -> Result<()> {
    let table = match blueprint.tables.get_mut(STRUCTURED_TABLE_ID) {
        Some(table) => table,
        None => return Ok(()),
    };
    let ordered = crate::ordered_columns(table)
        .into_iter()
        .map(|(name, col)| {
            (
                name.clone(),
                crate::type_tag_for_column(col),
                col.numeric_scale,
            )
        })
        .collect::<Vec<_>>();
    if ordered.is_empty() {
        return Ok(());
    }

    let file = open_preflighted_avro(path, "for decoded sampling")?;
    let reader =
        Reader::new(file).with_context(|| format!("reading Avro rows from {}", path.display()))?;
    let mut acc = CompressionSampleAccumulator::with_max_resident_bytes(
        ordered.len(),
        options.max_sample_bytes,
    )?;

    for value_result in reader.take(options.sample_rows as usize) {
        deadline.check("decoding an Avro sample row")?;
        let value = value_result.context("decoding Avro sample row")?;
        let Some(cells) = avro_value_to_cells_bounded(&value, &ordered, acc.max_input_row_bytes())
        else {
            break;
        };
        if !acc.push_row_bounded(&cells)? {
            break;
        }
    }

    let ratio_storage = table
        .compression
        .as_ref()
        .map(|compression| compression.ratio_storage)
        .unwrap_or_default();
    let (sampled_with_bias, bias_reason) = options.effective_bias(acc.sample_rows(), table.rows);
    if let Some(mut compression) = acc.table_compression_with_deadline(
        options.table_sample_method.clone(),
        deadline,
        sampled_with_bias,
        bias_reason,
    )? {
        compression.ratio_storage = ratio_storage;
        table.compression = Some(compression);
    }
    let column_compressions = acc.column_compressions_with_deadline(
        options.column_sample_method.clone(),
        deadline,
        sampled_with_bias,
        bias_reason,
    )?;
    let column_statistics = acc.column_statistics();
    let column_cardinalities = acc.column_cardinalities(
        table.rows,
        options.column_sample_method.as_str(),
        sampled_with_bias,
        bias_reason,
    );
    for ((((name, _tag, _scale), compression), statistics), cardinality) in ordered
        .iter()
        .zip(column_compressions.into_iter())
        .zip(column_statistics.into_iter())
        .zip(column_cardinalities.into_iter())
    {
        deadline.check("applying Avro sample column compression")?;
        if let Some(column) = table.cols.get_mut(name) {
            if let Some(compression) = compression {
                column.compression = Some(compression);
            }
            column.len_avg = statistics.len_avg;
            column.len_p95 = statistics.len_p95;
            column.null_fraction = Some(statistics.null_fraction);
            column.length_sample_rows = statistics.sample_rows;
            column.length_p95_sample_rows = statistics.len_p95_sample_rows;
            column.length_sample_method = options.column_sample_method.clone();
            column.cardinality = cardinality;
        }
    }
    deadline.check("finishing Avro decoded sampling")?;
    Ok(())
}

#[cfg(feature = "sampling")]
fn avro_value_to_cells_bounded(
    value: &Value,
    ordered: &[(String, TypeTag, u64)],
    max_input_bytes: usize,
) -> Option<Vec<OwnedCell>> {
    let cell_headers = ordered
        .len()
        .checked_mul(std::mem::size_of::<OwnedCell>())?;
    let mut remaining = max_input_bytes.checked_sub(cell_headers)?;
    if let Value::Record(fields) = value {
        let mut cells = Vec::with_capacity(ordered.len());
        for (idx, (_name, tag, scale)) in ordered.iter().enumerate() {
            let cell = match fields.get(idx) {
                Some((_source_name, value)) => {
                    avro_value_to_cell_bounded(value, *tag, *scale, remaining)?
                }
                None => OwnedCell::null(),
            };
            remaining = remaining.saturating_sub(
                cell.bytes
                    .as_ref()
                    .map_or(1, |bytes| bytes.len().saturating_add(1)),
            );
            cells.push(cell);
        }
        Some(cells)
    } else {
        let fallback = ordered
            .first()
            .map(|(_, tag, scale)| (*tag, *scale))
            .unwrap_or((TypeTag::UnknownText, 0));
        Some(vec![avro_value_to_cell_bounded(
            value, fallback.0, fallback.1, remaining,
        )?])
    }
}

#[cfg(feature = "sampling")]
fn avro_value_to_cell_bounded(
    value: &Value,
    fallback_tag: TypeTag,
    numeric_scale: u64,
    max_payload_bytes: usize,
) -> Option<OwnedCell> {
    let cell = match value {
        Value::Null => return Some(OwnedCell::null()),
        Value::Boolean(value) => OwnedCell::new(TypeTag::BoolText, value.to_string().into_bytes()),
        Value::Int(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Value::Long(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Value::Float(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Value::Double(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Value::Bytes(value) => {
            if value.len() > max_payload_bytes {
                return None;
            }
            OwnedCell::new(TypeTag::BinaryRaw, value.clone())
        }
        Value::Fixed(_, value) => {
            if value.len() > max_payload_bytes {
                return None;
            }
            OwnedCell::new(TypeTag::BinaryRaw, value.clone())
        }
        Value::String(value) => OwnedCell::new(
            if matches!(fallback_tag, TypeTag::JsonText) {
                TypeTag::JsonText
            } else {
                TypeTag::TextUtf8
            },
            {
                if value.len() > max_payload_bytes {
                    return None;
                }
                value.as_bytes().to_vec()
            },
        ),
        Value::Enum(_, value) => {
            if value.len() > max_payload_bytes {
                return None;
            }
            OwnedCell::new(TypeTag::TextUtf8, value.as_bytes().to_vec())
        }
        Value::Union(_, inner) => {
            return avro_value_to_cell_bounded(
                inner,
                fallback_tag,
                numeric_scale,
                max_payload_bytes,
            )
        }
        Value::Array(_) | Value::Map(_) | Value::Record(_) => {
            if avro_json_encoded_len(value) > max_payload_bytes {
                return None;
            }
            OwnedCell::new(TypeTag::JsonText, canonical_avro_json(value))
        }
        Value::Date(value) => OwnedCell::new(
            TypeTag::DateText,
            crate::canonical_date_days(*value).into_bytes(),
        ),
        Value::Decimal(value) => OwnedCell::new(
            TypeTag::NumberText,
            canonical_avro_decimal(value, numeric_scale).into_bytes(),
        ),
        Value::BigDecimal(value) => {
            OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes())
        }
        Value::TimeMillis(value) => OwnedCell::new(
            TypeTag::TimeText,
            crate::canonical_time_units(i64::from(*value), 1_000, 3).into_bytes(),
        ),
        Value::TimeMicros(value) => OwnedCell::new(
            TypeTag::TimeText,
            crate::canonical_time_units(*value, 1_000_000, 6).into_bytes(),
        ),
        Value::TimestampMillis(value) => OwnedCell::new(
            TypeTag::TimestampText,
            crate::canonical_timestamp_units(*value, 1_000, 3, true).into_bytes(),
        ),
        Value::TimestampMicros(value) => OwnedCell::new(
            TypeTag::TimestampText,
            crate::canonical_timestamp_units(*value, 1_000_000, 6, true).into_bytes(),
        ),
        Value::TimestampNanos(value) => OwnedCell::new(
            TypeTag::TimestampText,
            crate::canonical_timestamp_units(*value, 1_000_000_000, 9, true).into_bytes(),
        ),
        Value::LocalTimestampMillis(value) => OwnedCell::new(
            TypeTag::TimestampText,
            crate::canonical_timestamp_units(*value, 1_000, 3, false).into_bytes(),
        ),
        Value::LocalTimestampMicros(value) => OwnedCell::new(
            TypeTag::TimestampText,
            crate::canonical_timestamp_units(*value, 1_000_000, 6, false).into_bytes(),
        ),
        Value::LocalTimestampNanos(value) => OwnedCell::new(
            TypeTag::TimestampText,
            crate::canonical_timestamp_units(*value, 1_000_000_000, 9, false).into_bytes(),
        ),
        Value::Duration(value) => {
            OwnedCell::new(TypeTag::BinaryRaw, <[u8; 12]>::from(*value).to_vec())
        }
        Value::Uuid(value) => OwnedCell::new(TypeTag::UuidText, value.to_string().into_bytes()),
    };
    if cell
        .bytes
        .as_ref()
        .is_some_and(|bytes| bytes.len() > max_payload_bytes)
    {
        None
    } else {
        Some(cell)
    }
}

fn fill_avro_columns(schema: &Schema, table: &mut BlueprintTable) {
    match schema {
        Schema::Record(record) => {
            for (idx, field) in record.fields.iter().enumerate() {
                let (nullable, inner) = nullable_avro_schema(&field.schema);
                table.cols.insert(
                    format!("col-{}", idx + 1),
                    avro_column_blueprint(inner, nullable, (idx + 1) as u32),
                );
            }
        }
        other => {
            table.cols.insert(
                "col-1".to_string(),
                avro_column_blueprint(other, matches!(other, Schema::Null), 1),
            );
        }
    }
}

fn nullable_avro_schema(schema: &Schema) -> (bool, &Schema) {
    if let Schema::Union(union) = schema {
        let mut non_null = None;
        let mut non_null_count = 0usize;
        let mut nullable = false;
        for variant in union.variants() {
            if matches!(variant, Schema::Null) {
                nullable = true;
            } else if non_null.is_none() {
                non_null = Some(variant);
                non_null_count += 1;
            } else {
                non_null_count += 1;
            }
        }
        if non_null_count == 1 {
            (nullable, non_null.unwrap_or(schema))
        } else {
            (nullable, schema)
        }
    } else {
        (false, schema)
    }
}

fn avro_column_blueprint(schema: &Schema, nullable: bool, ordinal: u32) -> BlueprintColumn {
    let mut column = BlueprintColumn {
        ordinal,
        column_type: avro_type_label(schema),
        nullable,
        native_type: avro_native_type(schema),
        len_avg: default_avro_len(schema),
        style: avro_style_label(schema),
        compression: Some(BlueprintCompression {
            measured: false,
            sample_method: "avro-schema".to_string(),
            sample_encoding: "avro-schema".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    match schema {
        Schema::Decimal(decimal) => {
            column.numeric_precision = decimal.precision as u64;
            column.numeric_scale = decimal.scale as u64;
        }
        Schema::Fixed(fixed) => column.declared_max_bytes = fixed.size as u64,
        Schema::Uuid => {
            column.declared_max_chars = 36;
            column.declared_max_bytes = 36;
        }
        Schema::TimeMillis | Schema::TimestampMillis | Schema::LocalTimestampMillis => {
            column.datetime_precision = 3;
        }
        Schema::TimeMicros | Schema::TimestampMicros | Schema::LocalTimestampMicros => {
            column.datetime_precision = 6;
        }
        Schema::TimestampNanos | Schema::LocalTimestampNanos => {
            column.datetime_precision = 9;
        }
        Schema::Union(union)
            if union
                .variants()
                .iter()
                .filter(|variant| !matches!(variant, Schema::Null))
                .count()
                > 1 =>
        {
            column.source_semantics = "multi-type-union".to_string();
        }
        Schema::Array(_) | Schema::Map(_) | Schema::Record(_) => {
            column.source_semantics = "nested-json".to_string();
        }
        _ => {}
    }
    column
}

fn avro_type_label(schema: &Schema) -> String {
    match schema {
        Schema::Null => "null",
        Schema::Boolean => "boolean",
        Schema::Int => "int",
        Schema::Long => "bigint",
        Schema::Float => "float",
        Schema::Double => "double",
        Schema::Bytes | Schema::Fixed(_) => "bytes",
        Schema::String | Schema::Enum(_) => "string",
        Schema::Decimal(_) | Schema::BigDecimal => "decimal",
        Schema::Uuid => "uuid",
        Schema::Date => "date",
        Schema::TimeMillis | Schema::TimeMicros => "time",
        Schema::TimestampMillis
        | Schema::TimestampMicros
        | Schema::TimestampNanos
        | Schema::LocalTimestampMillis
        | Schema::LocalTimestampMicros
        | Schema::LocalTimestampNanos => "timestamp",
        Schema::Array(_) | Schema::Map(_) | Schema::Record(_) | Schema::Union(_) => "json",
        Schema::Duration => "bytes",
        Schema::Ref { .. } => "string",
    }
    .to_string()
}

fn avro_style_label(schema: &Schema) -> String {
    match schema {
        Schema::Array(_) | Schema::Map(_) | Schema::Record(_) | Schema::Union(_) => {
            "json".to_string()
        }
        Schema::String | Schema::Enum(_) | Schema::Uuid => "text".to_string(),
        _ => String::new(),
    }
}

fn avro_native_type(schema: &Schema) -> String {
    match schema {
        Schema::Decimal(decimal) => format!(
            "avro:decimal[precision={},scale={}]",
            decimal.precision, decimal.scale
        ),
        Schema::Fixed(fixed) => format!("avro:fixed[size={}]", fixed.size),
        Schema::TimestampMillis => "avro:timestamp-millis[utc=true]".to_string(),
        Schema::TimestampMicros => "avro:timestamp-micros[utc=true]".to_string(),
        Schema::TimestampNanos => "avro:timestamp-nanos[utc=true]".to_string(),
        Schema::LocalTimestampMillis => "avro:local-timestamp-millis".to_string(),
        Schema::LocalTimestampMicros => "avro:local-timestamp-micros".to_string(),
        Schema::LocalTimestampNanos => "avro:local-timestamp-nanos".to_string(),
        Schema::TimeMillis => "avro:time-millis".to_string(),
        Schema::TimeMicros => "avro:time-micros".to_string(),
        Schema::Union(union) => format!(
            "avro:union[{}]",
            union
                .variants()
                .iter()
                .map(avro_type_label)
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => format!("avro:{}", avro_type_label(other)),
    }
}

fn default_avro_len(schema: &Schema) -> u64 {
    match schema {
        Schema::Boolean => 1,
        Schema::Int | Schema::Float | Schema::Date | Schema::TimeMillis => 4,
        Schema::Long
        | Schema::Double
        | Schema::TimeMicros
        | Schema::TimestampMillis
        | Schema::TimestampMicros
        | Schema::TimestampNanos
        | Schema::LocalTimestampMillis
        | Schema::LocalTimestampMicros
        | Schema::LocalTimestampNanos => 8,
        Schema::Bytes | Schema::String | Schema::Enum(_) | Schema::Fixed(_) | Schema::Uuid => 64,
        Schema::Decimal(_) | Schema::BigDecimal => 32,
        Schema::Array(_) | Schema::Map(_) | Schema::Record(_) | Schema::Union(_) => 256,
        Schema::Null => 0,
        Schema::Duration => 12,
        Schema::Ref { .. } => 64,
    }
}

const WIDTH_RESERVOIR: usize = 8_192;

fn width_reservoir_capacity(column_count: usize, max_resident_bytes: usize) -> Result<usize> {
    let fixed_bytes = column_count
        .checked_mul(std::mem::size_of::<ObservedColumnStats>())
        .context("Avro observed-column metadata size overflow")?;
    if fixed_bytes > max_resident_bytes {
        anyhow::bail!(
            "Avro Blueprint scan needs at least {fixed_bytes} bytes for {column_count} columns, above the {max_resident_bytes} byte resident-memory budget"
        );
    }
    if column_count == 0 {
        return Ok(0);
    }
    Ok(
        ((max_resident_bytes - fixed_bytes) / column_count / std::mem::size_of::<u64>())
            .min(WIDTH_RESERVOIR),
    )
}

#[derive(Debug)]
struct ObservedColumnStats {
    rows: u64,
    non_null: u64,
    nulls: u64,
    total_bytes: u64,
    lengths: Vec<u64>,
    max_lengths: usize,
}

impl ObservedColumnStats {
    fn new(max_lengths: usize) -> Self {
        Self {
            rows: 0,
            non_null: 0,
            nulls: 0,
            total_bytes: 0,
            lengths: Vec::new(),
            max_lengths,
        }
    }

    fn push(&mut self, value: Option<&Value>, numeric_scale: u64) -> u64 {
        self.rows = self.rows.saturating_add(1);
        let Some(value) = value.and_then(unwrap_avro_union) else {
            self.nulls = self.nulls.saturating_add(1);
            return 1;
        };
        if matches!(value, Value::Null) {
            self.nulls = self.nulls.saturating_add(1);
            return 1;
        }
        let length = avro_value_payload_len(value, numeric_scale);
        self.non_null = self.non_null.saturating_add(1);
        self.total_bytes = self.total_bytes.saturating_add(length);
        if self.lengths.len() < self.max_lengths {
            self.lengths.push(length);
        } else if self.max_lengths > 0 {
            let candidate = sample_mix64(self.non_null) % self.non_null.max(1);
            if candidate < self.max_lengths as u64 {
                self.lengths[candidate as usize] = length;
            }
        }
        1 + varint_len(length) + length
    }

    fn null_fraction(&self) -> f64 {
        if self.rows == 0 {
            0.0
        } else {
            self.nulls as f64 / self.rows as f64
        }
    }

    fn len_avg(&self) -> u64 {
        if self.non_null == 0 {
            0
        } else {
            self.total_bytes.saturating_add(self.non_null / 2) / self.non_null
        }
    }

    fn len_p95(&self) -> u64 {
        if self.lengths.is_empty() {
            return 0;
        }
        let mut lengths = self.lengths.clone();
        lengths.sort_unstable();
        let rank = ((lengths.len() as f64 * 0.95).ceil() as usize)
            .saturating_sub(1)
            .min(lengths.len() - 1);
        lengths[rank]
    }
}

fn observe_avro_row(
    value: &Value,
    observed: &mut [ObservedColumnStats],
    columns: &[BlueprintColumn],
) -> u64 {
    if let Value::Record(fields) = value {
        observed
            .iter_mut()
            .enumerate()
            .map(|(idx, stats)| {
                stats.push(
                    fields.get(idx).map(|(_name, value)| value),
                    columns.get(idx).map_or(0, |column| column.numeric_scale),
                )
            })
            .sum()
    } else {
        observed
            .first_mut()
            .map(|stats| {
                stats.push(
                    Some(value),
                    columns.first().map_or(0, |column| column.numeric_scale),
                )
            })
            .unwrap_or(0)
    }
}

fn unwrap_avro_union(value: &Value) -> Option<&Value> {
    match value {
        Value::Union(_, inner) => unwrap_avro_union(inner),
        Value::Null => None,
        other => Some(other),
    }
}

fn avro_value_payload_len(value: &Value, numeric_scale: u64) -> u64 {
    match value {
        Value::Null => 0,
        Value::Boolean(value) => value.to_string().len() as u64,
        Value::Int(value) => value.to_string().len() as u64,
        Value::Long(value) => value.to_string().len() as u64,
        Value::Float(value) => value.to_string().len() as u64,
        Value::Double(value) => value.to_string().len() as u64,
        Value::Bytes(value) | Value::Fixed(_, value) => value.len() as u64,
        Value::String(value) | Value::Enum(_, value) => value.len() as u64,
        Value::Union(_, inner) => avro_value_payload_len(inner, numeric_scale),
        Value::Date(value) => crate::canonical_date_days(*value).len() as u64,
        Value::TimeMillis(value) => {
            crate::canonical_time_units(i64::from(*value), 1_000, 3).len() as u64
        }
        Value::TimeMicros(value) => crate::canonical_time_units(*value, 1_000_000, 6).len() as u64,
        Value::TimestampMillis(value) => {
            crate::canonical_timestamp_units(*value, 1_000, 3, true).len() as u64
        }
        Value::TimestampMicros(value) => {
            crate::canonical_timestamp_units(*value, 1_000_000, 6, true).len() as u64
        }
        Value::TimestampNanos(value) => {
            crate::canonical_timestamp_units(*value, 1_000_000_000, 9, true).len() as u64
        }
        Value::LocalTimestampMillis(value) => {
            crate::canonical_timestamp_units(*value, 1_000, 3, false).len() as u64
        }
        Value::LocalTimestampMicros(value) => {
            crate::canonical_timestamp_units(*value, 1_000_000, 6, false).len() as u64
        }
        Value::LocalTimestampNanos(value) => {
            crate::canonical_timestamp_units(*value, 1_000_000_000, 9, false).len() as u64
        }
        Value::BigDecimal(value) => value.to_string().len() as u64,
        Value::Uuid(value) => value.to_string().len() as u64,
        Value::Decimal(value) => canonical_avro_decimal(value, numeric_scale).len() as u64,
        Value::Duration(_) => 12,
        Value::Array(_) | Value::Map(_) | Value::Record(_) => {
            avro_json_encoded_len(value).min(u64::MAX as usize) as u64
        }
    }
}

fn avro_json_encoded_len(value: &Value) -> usize {
    match value {
        Value::Null => 4,
        Value::Boolean(true) => 4,
        Value::Boolean(false) => 5,
        Value::Int(value) => value.to_string().len(),
        Value::Long(value) => value.to_string().len(),
        Value::Float(value) => {
            if value.is_finite() {
                value.to_string().len()
            } else {
                json_string_len(&value.to_string())
            }
        }
        Value::Double(value) => {
            if value.is_finite() {
                value.to_string().len()
            } else {
                json_string_len(&value.to_string())
            }
        }
        Value::Bytes(value) | Value::Fixed(_, value) => {
            value.len().saturating_mul(2).saturating_add(2)
        }
        Value::String(value) | Value::Enum(_, value) => json_string_len(value),
        Value::Union(_, inner) => avro_json_encoded_len(inner),
        Value::Array(values) => {
            collection_json_len(values.iter().map(avro_json_encoded_len), values.len(), 2)
        }
        Value::Map(values) => collection_json_len(
            values.iter().map(|(key, value)| {
                json_string_len(key)
                    .saturating_add(1)
                    .saturating_add(avro_json_encoded_len(value))
            }),
            values.len(),
            2,
        ),
        Value::Record(fields) => collection_json_len(
            fields.iter().map(|(key, value)| {
                json_string_len(key)
                    .saturating_add(1)
                    .saturating_add(avro_json_encoded_len(value))
            }),
            fields.len(),
            2,
        ),
        other => avro_json_value(other).to_string().len(),
    }
}

fn collection_json_len(
    values: impl Iterator<Item = usize>,
    count: usize,
    delimiters: usize,
) -> usize {
    values.fold(
        delimiters.saturating_add(count.saturating_sub(1)),
        |total, value| total.saturating_add(value),
    )
}

fn json_string_len(value: &str) -> usize {
    value.chars().fold(2usize, |total, character| {
        total.saturating_add(match character {
            '"' | '\\' | '\u{0008}' | '\u{000C}' | '\n' | '\r' | '\t' => 2,
            character if character <= '\u{001F}' => 6,
            character => character.len_utf8(),
        })
    })
}

fn canonical_avro_decimal(value: &apache_avro::Decimal, scale: u64) -> String {
    use num_bigint::{BigInt, Sign};

    let bytes = Vec::<u8>::try_from(value).unwrap_or_default();
    let integer = BigInt::from_signed_bytes_be(&bytes);
    let negative = integer.sign() == Sign::Minus;
    let mut digits = integer.magnitude().to_str_radix(10);
    let scale = usize::try_from(scale.min(1_000)).unwrap_or(0);
    if scale > 0 {
        if digits.len() <= scale {
            digits.insert_str(0, &"0".repeat(scale + 1 - digits.len()));
        }
        digits.insert(digits.len() - scale, '.');
    }
    if negative {
        digits.insert(0, '-');
    }
    digits
}

#[cfg(any(feature = "sampling", test))]
fn canonical_avro_json(value: &Value) -> Vec<u8> {
    serde_json::to_vec(&avro_json_value(value)).unwrap_or_else(|_| b"null".to_vec())
}

fn avro_json_value(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(value) => serde_json::Value::Bool(*value),
        Value::Int(value) => serde_json::Value::Number((*value).into()),
        Value::Long(value) => serde_json::Value::Number((*value).into()),
        Value::Float(value) => serde_json::Number::from_f64(f64::from(*value))
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(value.to_string())),
        Value::Double(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(value.to_string())),
        Value::Bytes(value) | Value::Fixed(_, value) => serde_json::Value::String(hex_bytes(value)),
        Value::String(value) | Value::Enum(_, value) => serde_json::Value::String(value.clone()),
        Value::Union(_, inner) => avro_json_value(inner),
        Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(avro_json_value).collect())
        }
        Value::Map(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), avro_json_value(value)))
                .collect(),
        ),
        Value::Record(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), avro_json_value(value)))
                .collect(),
        ),
        Value::Date(value) => serde_json::Value::String(crate::canonical_date_days(*value)),
        Value::Decimal(value) => serde_json::Value::String(canonical_avro_decimal(value, 0)),
        Value::BigDecimal(value) => serde_json::Value::String(value.to_string()),
        Value::TimeMillis(value) => {
            serde_json::Value::String(crate::canonical_time_units(i64::from(*value), 1_000, 3))
        }
        Value::TimeMicros(value) => {
            serde_json::Value::String(crate::canonical_time_units(*value, 1_000_000, 6))
        }
        Value::TimestampMillis(value) => {
            serde_json::Value::String(crate::canonical_timestamp_units(*value, 1_000, 3, true))
        }
        Value::TimestampMicros(value) => {
            serde_json::Value::String(crate::canonical_timestamp_units(*value, 1_000_000, 6, true))
        }
        Value::TimestampNanos(value) => serde_json::Value::String(
            crate::canonical_timestamp_units(*value, 1_000_000_000, 9, true),
        ),
        Value::LocalTimestampMillis(value) => {
            serde_json::Value::String(crate::canonical_timestamp_units(*value, 1_000, 3, false))
        }
        Value::LocalTimestampMicros(value) => serde_json::Value::String(
            crate::canonical_timestamp_units(*value, 1_000_000, 6, false),
        ),
        Value::LocalTimestampNanos(value) => serde_json::Value::String(
            crate::canonical_timestamp_units(*value, 1_000_000_000, 9, false),
        ),
        Value::Duration(value) => serde_json::json!({
            "months": u32::from(value.months()),
            "days": u32::from(value.days()),
            "millis": u32::from(value.millis()),
        }),
        Value::Uuid(value) => serde_json::Value::String(value.to_string()),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn varint_len(mut value: u64) -> u64 {
    let mut bytes = 1;
    while value >= 0x80 {
        value >>= 7;
        bytes += 1;
    }
    bytes
}

fn sample_mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn compression_ratio(uncompressed: u64, compressed: u64) -> f64 {
    if uncompressed == 0 || compressed == 0 {
        0.0
    } else {
        uncompressed as f64 / compressed as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apache_avro::Writer;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tmp")
            .join("blueprint-core-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{name}-{nonce}.avro"))
    }

    fn encode_avro_long(value: i64, output: &mut Vec<u8>) {
        let mut encoded = ((value as u64) << 1) ^ ((value >> 63) as u64);
        loop {
            let mut byte = (encoded & 0x7f) as u8;
            encoded >>= 7;
            if encoded != 0 {
                byte |= 0x80;
            }
            output.push(byte);
            if encoded == 0 {
                break;
            }
        }
    }

    fn avro_container_with_schema(schema: &[u8]) -> Vec<u8> {
        let mut bytes = AVRO_MAGIC.to_vec();
        encode_avro_long(1, &mut bytes);
        encode_avro_long("avro.schema".len() as i64, &mut bytes);
        bytes.extend_from_slice(b"avro.schema");
        encode_avro_long(schema.len() as i64, &mut bytes);
        bytes.extend_from_slice(schema);
        encode_avro_long(0, &mut bytes);
        bytes.extend_from_slice(&[0u8; 16]);
        bytes
    }

    #[test]
    fn avro_schema_depth_is_rejected_before_upstream_reader() {
        let mut schema = vec![b'['; MAX_AVRO_SCHEMA_DEPTH + 1];
        schema.extend_from_slice(b"\"null\"");
        schema.extend(std::iter::repeat_n(b']', MAX_AVRO_SCHEMA_DEPTH + 1));
        let path = test_path("deep-schema");
        std::fs::write(&path, avro_container_with_schema(&schema)).unwrap();

        let error = avro_blueprint_from_path(&path).unwrap_err();
        assert!(error.to_string().contains("checking Avro schema safety"));
        assert!(format!("{error:#}").contains("schema nesting exceeds"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn avro_schema_depth_scanner_ignores_delimiters_in_strings() {
        validate_avro_schema_nesting(br#"{"type":"record","name":"[[{{secret}}]]","fields":[]}"#)
            .unwrap();
    }

    #[test]
    fn avro_schema_depth_limit_has_an_exact_boundary() {
        let mut accepted = vec![b'['; MAX_AVRO_SCHEMA_DEPTH];
        accepted.extend_from_slice(b"\"null\"");
        accepted.extend(std::iter::repeat_n(b']', MAX_AVRO_SCHEMA_DEPTH));
        validate_avro_schema_nesting(&accepted).unwrap();

        let mut rejected = vec![b'['; MAX_AVRO_SCHEMA_DEPTH + 1];
        rejected.extend_from_slice(b"\"null\"");
        rejected.extend(std::iter::repeat_n(b']', MAX_AVRO_SCHEMA_DEPTH + 1));
        assert!(validate_avro_schema_nesting(&rejected).is_err());
    }

    #[test]
    fn nullable_union_maps_to_inner_type() {
        let schema = Schema::parse_str(r#"["null", "string"]"#).unwrap();
        let (nullable, inner) = nullable_avro_schema(&schema);
        assert!(nullable);
        assert_eq!(avro_type_label(inner), "string");
    }

    #[test]
    fn multi_type_union_is_explicit_json_semantics() {
        let schema = Schema::parse_str(r#"["null", "string", "long"]"#).unwrap();
        let (nullable, inner) = nullable_avro_schema(&schema);
        let column = avro_column_blueprint(inner, nullable, 1);
        assert!(nullable);
        assert_eq!(column.column_type, "json");
        assert_eq!(column.source_semantics, "multi-type-union");
    }

    #[test]
    fn logical_schema_metadata_preserves_precision_and_time_semantics() {
        let decimal = Schema::parse_str(
            r#"{"type":"bytes","logicalType":"decimal","precision":18,"scale":5}"#,
        )
        .unwrap();
        let decimal_column = avro_column_blueprint(&decimal, false, 1);
        assert_eq!(decimal_column.numeric_precision, 18);
        assert_eq!(decimal_column.numeric_scale, 5);

        let timestamp =
            Schema::parse_str(r#"{"type":"long","logicalType":"local-timestamp-micros"}"#).unwrap();
        let timestamp_column = avro_column_blueprint(&timestamp, false, 1);
        assert_eq!(timestamp_column.datetime_precision, 6);
        assert!(timestamp_column.native_type.contains("local-timestamp"));
    }

    #[test]
    fn avro_width_reservoir_rejects_wide_schemas_before_allocation() {
        assert!(width_reservoir_capacity(100_000, 1024).is_err());
        assert_eq!(width_reservoir_capacity(0, 1).unwrap(), 0);
    }

    #[cfg(feature = "sampling")]
    #[test]
    fn oversized_avro_values_are_rejected_before_sample_copy() {
        let value = Value::String("界".repeat(1024));
        assert!(avro_value_to_cell_bounded(&value, TypeTag::TextUtf8, 0, 64).is_none());
    }

    #[test]
    fn bounded_json_length_matches_canonical_encoding() {
        let value = Value::Record(vec![
            ("message".into(), Value::String("line\n界".into())),
            (
                "values".into(),
                Value::Array(vec![Value::Long(1), Value::Boolean(true)]),
            ),
        ]);
        assert_eq!(
            avro_json_encoded_len(&value),
            canonical_avro_json(&value).len()
        );
    }

    #[test]
    fn avro_full_scan_populates_logical_bytes_widths_and_nulls() {
        let schema = Schema::parse_str(
            r#"{
                "type":"record","name":"event","fields":[
                  {"name":"name","type":"string"},
                  {"name":"payload","type":"bytes"},
                  {"name":"optional","type":["null","string"],"default":null}
                ]
            }"#,
        )
        .unwrap();
        let path = test_path("observed");
        let file = File::create(&path).unwrap();
        let mut writer = Writer::new(&schema, file);
        writer
            .append(Value::Record(vec![
                ("name".to_string(), Value::String("alpha".to_string())),
                ("payload".to_string(), Value::Bytes(vec![1, 2, 3])),
                (
                    "optional".to_string(),
                    Value::Union(0, Box::new(Value::Null)),
                ),
            ]))
            .unwrap();
        writer
            .append(Value::Record(vec![
                ("name".to_string(), Value::String("longer-name".to_string())),
                ("payload".to_string(), Value::Bytes(vec![4; 9])),
                (
                    "optional".to_string(),
                    Value::Union(1, Box::new(Value::String("present".to_string()))),
                ),
            ]))
            .unwrap();
        writer.flush().unwrap();
        drop(writer);

        let expired = SamplingDeadline::after(std::time::Duration::ZERO);
        let error = avro_blueprint_from_path_with_deadline(&path, &expired)
            .expect_err("caller-owned Avro deadline must be honored");
        assert!(error.to_string().contains("deadline expired"));

        let blueprint = avro_blueprint_from_path(&path).unwrap();
        let table = blueprint.tables.get(STRUCTURED_TABLE_ID).unwrap();
        assert_eq!(table.rows, 2);
        assert!(table.table_bytes > 0);
        assert!(table.storage_bytes > 0);
        assert!(table.source_codec.is_empty());
        assert_eq!(blueprint.totals.table_bytes, table.table_bytes);
        assert_eq!(table.cols["col-1"].len_avg, 8);
        assert_eq!(table.cols["col-1"].len_p95, 11);
        assert_eq!(table.cols["col-3"].null_fraction, Some(0.5));
        let compression = table.compression.as_ref().unwrap();
        assert_eq!(compression.sample_encoding, "avro-container");
        assert_eq!(compression.ratio_zstd_3, 0.0);
        assert!(compression.ratio_storage > 0.0);
        let blueprint_toml = toml::to_string_pretty(&blueprint).unwrap();
        assert!(blueprint_toml.contains("storage_bytes ="));
        assert!(blueprint_toml.contains("ratio_storage ="));
        assert!(blueprint_toml.contains("null_fraction = 0.5"));
        assert!(!blueprint_toml.contains("cols.name"));
        assert!(!blueprint_toml.contains("cols.payload"));
        assert!(!blueprint_toml.contains("cols.optional"));
        assert!(blueprint_toml.contains("type = \"string\""));
        assert!(!blueprint_toml.contains("column_type ="));
        assert!(!blueprint_toml.contains("ratio_zstd_3 ="));

        #[cfg(feature = "sampling")]
        {
            let sampled_blueprint = avro_blueprint_from_path_with_options(
                &path,
                &DecodedCompressionOptions::enabled(2, "table-test", "column-test"),
            )
            .unwrap();
            let sampled_compression = sampled_blueprint.tables[STRUCTURED_TABLE_ID]
                .compression
                .as_ref()
                .unwrap();
            assert_eq!(
                sampled_compression.sample_encoding,
                crate::SAMPLE_ENCODING_TAG
            );
            assert!(sampled_compression.ratio_zstd_3 > 0.0);
            assert!(sampled_compression.ratio_storage > 0.0);
            assert!(!sampled_compression.sampled_with_bias);
            assert!(sampled_compression.bias_reason.is_empty());

            let partial_blueprint = avro_blueprint_from_path_with_options(
                &path,
                &DecodedCompressionOptions::enabled(1, "table-first", "column-first"),
            )
            .unwrap();
            let partial = partial_blueprint.tables[STRUCTURED_TABLE_ID]
                .compression
                .as_ref()
                .unwrap();
            assert!(partial.sampled_with_bias);
            assert_eq!(partial.bias_reason, crate::FIRST_N_BIAS_REASON);
        }

        std::fs::remove_file(path).unwrap();
    }

    #[cfg(feature = "sampling")]
    #[test]
    fn one_deadline_covers_avro_metadata_and_sampling() {
        let schema = Schema::parse_str(
            r#"{"type":"record","name":"event","fields":[{"name":"id","type":"long"}]}"#,
        )
        .unwrap();
        let path = test_path("deadline");
        let file = File::create(&path).unwrap();
        let mut writer = Writer::new(&schema, file);
        writer
            .append(Value::Record(vec![("id".into(), Value::Long(1))]))
            .unwrap();
        writer.flush().unwrap();
        drop(writer);

        let mut options = DecodedCompressionOptions::enabled(1, "table", "column");
        options.max_wall = std::time::Duration::ZERO;
        let deadline = options.deadline();
        let error = avro_blueprint_from_path_with_options_and_deadline(&path, &options, &deadline)
            .expect_err("an already-expired shared deadline must fail metadata");
        assert!(error.to_string().contains("deadline expired"));
        std::fs::remove_file(path).unwrap();
    }
}
