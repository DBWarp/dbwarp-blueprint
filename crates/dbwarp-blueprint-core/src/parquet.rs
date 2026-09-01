use crate::{
    BlueprintColumn, BlueprintCompression, BlueprintFile, BlueprintTable, SamplingDeadline, Totals,
    SCHEMA_VERSION,
};
use anyhow::{Context, Result};
use parquet::basic::{ConvertedType, LogicalType, TimeUnit};
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::schema::types::ColumnDescriptor;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[cfg(feature = "sampling")]
use crate::{CompressionSampleAccumulator, DecodedCompressionOptions, OwnedCell, TypeTag};
#[cfg(feature = "sampling")]
use parquet::record::Field;

const STRUCTURED_TABLE_ID: &str = "table-001";
const PARQUET_MAGIC: &[u8; 4] = b"PAR1";
const PARQUET_FOOTER_TRAILER_BYTES: u64 = 8;
const MAX_PARQUET_FOOTER_BYTES: usize = 64 * 1024 * 1024;
const MAX_PARQUET_SCHEMA_ELEMENTS: usize = 1_000_000;
const MAX_PARQUET_SCHEMA_DEPTH: usize = 64;
const MAX_COMPACT_NESTING: usize = 16;
const MAX_COMPACT_COLLECTION_ITEMS: usize = 1_000_000;

// Compact-Thrift type tags used by the Parquet footer. This preflight parser
// intentionally extracts only SchemaElement.num_children. It therefore avoids
// constructing the recursive Parquet schema until its depth has been bounded.
const COMPACT_STOP: u8 = 0;
const COMPACT_BOOL_TRUE: u8 = 1;
const COMPACT_BOOL_FALSE: u8 = 2;
const COMPACT_BYTE: u8 = 3;
const COMPACT_I16: u8 = 4;
const COMPACT_I32: u8 = 5;
const COMPACT_I64: u8 = 6;
const COMPACT_DOUBLE: u8 = 7;
const COMPACT_BINARY: u8 = 8;
const COMPACT_LIST: u8 = 9;
const COMPACT_SET: u8 = 10;
const COMPACT_MAP: u8 = 11;
const COMPACT_STRUCT: u8 = 12;

struct CompactFooter<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> CompactFooter<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn read_byte(&mut self) -> Result<u8> {
        let byte = *self
            .bytes
            .get(self.position)
            .context("Parquet footer ended unexpectedly")?;
        self.position += 1;
        Ok(byte)
    }

    fn advance(&mut self, count: usize) -> Result<()> {
        let end = self
            .position
            .checked_add(count)
            .context("Parquet footer offset overflowed")?;
        if end > self.bytes.len() {
            anyhow::bail!("Parquet footer value exceeds the available metadata bytes");
        }
        self.position = end;
        Ok(())
    }

    fn read_varint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for index in 0..10 {
            let byte = self.read_byte()?;
            if index == 9 && (byte & 0xfe) != 0 {
                anyhow::bail!("Parquet footer contains an out-of-range varint");
            }
            value |= u64::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        anyhow::bail!("Parquet footer contains an unterminated varint")
    }

    fn read_zigzag(&mut self) -> Result<i64> {
        let encoded = self.read_varint()?;
        Ok(((encoded >> 1) as i64) ^ (-((encoded & 1) as i64)))
    }

    fn read_field(&mut self, previous_id: &mut i16) -> Result<Option<(i16, u8)>> {
        let header = self.read_byte()?;
        let field_type = header & 0x0f;
        if field_type == COMPACT_STOP {
            return Ok(None);
        }
        if field_type > COMPACT_STRUCT {
            anyhow::bail!("Parquet footer contains an unknown Compact-Thrift type");
        }
        let delta = i16::from(header >> 4);
        let field_id = if delta == 0 {
            i16::try_from(self.read_zigzag()?)
                .context("Parquet footer field identifier is out of range")?
        } else {
            previous_id
                .checked_add(delta)
                .context("Parquet footer field identifier overflowed")?
        };
        *previous_id = field_id;
        Ok(Some((field_id, field_type)))
    }

    fn read_collection_header(&mut self) -> Result<(usize, u8)> {
        let header = self.read_byte()?;
        let encoded_count = usize::from(header >> 4);
        let count = if encoded_count == 15 {
            usize::try_from(self.read_varint()?)
                .context("Parquet footer collection count is unsupported")?
        } else {
            encoded_count
        };
        if count > MAX_COMPACT_COLLECTION_ITEMS || count > self.remaining() {
            anyhow::bail!("Parquet footer collection exceeds its safety limit");
        }
        let element_type = header & 0x0f;
        if element_type == COMPACT_STOP || element_type > COMPACT_STRUCT {
            anyhow::bail!("Parquet footer collection has an invalid element type");
        }
        Ok((count, element_type))
    }

    fn skip_field_value(&mut self, field_type: u8, depth: usize) -> Result<()> {
        if matches!(field_type, COMPACT_BOOL_TRUE | COMPACT_BOOL_FALSE) {
            return Ok(());
        }
        self.skip_value(field_type, depth)
    }

    fn skip_value(&mut self, value_type: u8, depth: usize) -> Result<()> {
        if depth > MAX_COMPACT_NESTING {
            anyhow::bail!("Parquet footer metadata nesting exceeds its safety limit");
        }
        match value_type {
            COMPACT_BOOL_TRUE | COMPACT_BOOL_FALSE | COMPACT_BYTE => self.advance(1),
            COMPACT_I16 | COMPACT_I32 | COMPACT_I64 => self.read_varint().map(|_| ()),
            COMPACT_DOUBLE => self.advance(8),
            COMPACT_BINARY => {
                let length = usize::try_from(self.read_varint()?)
                    .context("Parquet footer binary length is unsupported")?;
                self.advance(length)
            }
            COMPACT_LIST | COMPACT_SET => {
                let (count, element_type) = self.read_collection_header()?;
                for _ in 0..count {
                    self.skip_value(element_type, depth + 1)?;
                }
                Ok(())
            }
            COMPACT_MAP => {
                let count = usize::try_from(self.read_varint()?)
                    .context("Parquet footer map count is unsupported")?;
                if count > MAX_COMPACT_COLLECTION_ITEMS || count > self.remaining() {
                    anyhow::bail!("Parquet footer map exceeds its safety limit");
                }
                if count == 0 {
                    return Ok(());
                }
                let types = self.read_byte()?;
                let key_type = types >> 4;
                let value_type = types & 0x0f;
                if key_type == COMPACT_STOP
                    || value_type == COMPACT_STOP
                    || key_type > COMPACT_STRUCT
                    || value_type > COMPACT_STRUCT
                {
                    anyhow::bail!("Parquet footer map has an invalid element type");
                }
                for _ in 0..count {
                    self.skip_value(key_type, depth + 1)?;
                    self.skip_value(value_type, depth + 1)?;
                }
                Ok(())
            }
            COMPACT_STRUCT => {
                let mut previous_id = 0i16;
                while let Some((_field_id, field_type)) = self.read_field(&mut previous_id)? {
                    self.skip_field_value(field_type, depth + 1)?;
                }
                Ok(())
            }
            _ => anyhow::bail!("Parquet footer contains an unsupported Compact-Thrift type"),
        }
    }

    fn read_schema_element_children(&mut self) -> Result<Option<i32>> {
        let mut previous_id = 0i16;
        let mut num_children = None;
        while let Some((field_id, field_type)) = self.read_field(&mut previous_id)? {
            if field_id == 5 {
                if field_type != COMPACT_I32 {
                    anyhow::bail!("Parquet schema child count has the wrong encoded type");
                }
                let value = i32::try_from(self.read_zigzag()?)
                    .context("Parquet schema child count is out of range")?;
                num_children = Some(value);
            } else {
                self.skip_field_value(field_type, 1)?;
            }
        }
        Ok(num_children)
    }

    fn read_schema_child_counts(&mut self) -> Result<Vec<Option<i32>>> {
        let mut previous_id = 0i16;
        while let Some((field_id, field_type)) = self.read_field(&mut previous_id)? {
            if field_id != 2 {
                self.skip_field_value(field_type, 1)?;
                continue;
            }
            if field_type != COMPACT_LIST {
                anyhow::bail!("Parquet footer schema has the wrong encoded type");
            }
            let (count, element_type) = self.read_collection_header()?;
            if element_type != COMPACT_STRUCT {
                anyhow::bail!("Parquet footer schema elements have the wrong encoded type");
            }
            if count == 0 || count > MAX_PARQUET_SCHEMA_ELEMENTS {
                anyhow::bail!("Parquet schema element count exceeds its safety limit or is empty");
            }
            let mut children = Vec::with_capacity(count);
            for _ in 0..count {
                children.push(self.read_schema_element_children()?);
            }
            return Ok(children);
        }
        anyhow::bail!("Parquet footer does not contain a schema")
    }
}

fn validate_parquet_schema_depth(children: &[Option<i32>]) -> Result<()> {
    let mut ancestors_remaining = Vec::<usize>::new();
    for (index, child_count) in children.iter().enumerate() {
        let depth = if index == 0 {
            1
        } else {
            if ancestors_remaining.is_empty() {
                anyhow::bail!("Parquet schema contains elements outside its root tree");
            }
            ancestors_remaining.len() + 1
        };
        if depth > MAX_PARQUET_SCHEMA_DEPTH {
            anyhow::bail!(
                "Parquet schema nesting exceeds the {MAX_PARQUET_SCHEMA_DEPTH}-level safety limit"
            );
        }
        if index > 0 {
            let remaining = ancestors_remaining
                .last_mut()
                .context("Parquet schema parent stack is empty")?;
            *remaining = remaining
                .checked_sub(1)
                .context("Parquet schema parent has too many children")?;
        }
        let child_count = child_count.unwrap_or(0);
        let child_count = usize::try_from(child_count)
            .context("Parquet schema contains a negative child count")?;
        if child_count > 0 {
            ancestors_remaining.push(child_count);
        }
        while ancestors_remaining.last() == Some(&0) {
            ancestors_remaining.pop();
        }
    }
    if !ancestors_remaining.is_empty() {
        anyhow::bail!("Parquet schema ends before all declared children are present");
    }
    Ok(())
}

fn preflight_parquet_file(file: &mut File) -> Result<()> {
    let file_size = file.metadata().context("reading Parquet file size")?.len();
    if file_size < 12 {
        anyhow::bail!("Parquet file is too short to contain a header and footer");
    }
    file.seek(SeekFrom::End(-(PARQUET_FOOTER_TRAILER_BYTES as i64)))
        .context("seeking to Parquet footer trailer")?;
    let mut trailer = [0u8; PARQUET_FOOTER_TRAILER_BYTES as usize];
    file.read_exact(&mut trailer)
        .context("reading Parquet footer trailer")?;
    if &trailer[4..] != PARQUET_MAGIC {
        anyhow::bail!("Parquet footer magic is invalid or encrypted");
    }
    let encoded_length: [u8; 4] = trailer[..4]
        .try_into()
        .context("reading Parquet footer length")?;
    let metadata_length = usize::try_from(u32::from_le_bytes(encoded_length))
        .context("Parquet footer length is unsupported on this platform")?;
    if metadata_length > MAX_PARQUET_FOOTER_BYTES {
        anyhow::bail!("Parquet footer exceeds the {MAX_PARQUET_FOOTER_BYTES}-byte safety limit");
    }
    let metadata_length_u64 = metadata_length as u64;
    if metadata_length_u64 > file_size.saturating_sub(12) {
        anyhow::bail!("Parquet footer length exceeds the file bounds");
    }
    let metadata_start = file_size - PARQUET_FOOTER_TRAILER_BYTES - metadata_length_u64;
    file.seek(SeekFrom::Start(metadata_start))
        .context("seeking to Parquet footer metadata")?;
    let mut footer = vec![0u8; metadata_length];
    file.read_exact(&mut footer)
        .context("reading bounded Parquet footer metadata")?;
    let children = CompactFooter::new(&footer).read_schema_child_counts()?;
    validate_parquet_schema_depth(&children)
}

fn open_preflighted_parquet(path: &Path, purpose: &str) -> Result<File> {
    let mut file = File::open(path)
        .with_context(|| format!("opening Parquet file {} {purpose}", path.display()))?;
    preflight_parquet_file(&mut file)
        .with_context(|| format!("checking Parquet schema safety in {}", path.display()))?;
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("rewinding Parquet file {} {purpose}", path.display()))?;
    Ok(file)
}

/// Build a DBWarp Blueprint model from Parquet footer and row-group metadata.
///
/// This does not read row data. It gives the estimator/generator a cheap,
/// deterministic view of row counts, column types, nullability and stored-file
/// compression ratios before a full Parquet ingest path is selected.
pub fn parquet_blueprint_from_path(path: impl AsRef<Path>) -> Result<BlueprintFile> {
    parquet_blueprint_from_path_with_deadline(path, &SamplingDeadline::unlimited())
}

/// Build a Parquet Blueprint while honoring an operation-wide deadline supplied
/// by the caller. Reuse the same deadline for every file in a batch so the
/// wall-clock budget is not restarted at file boundaries.
pub fn parquet_blueprint_from_path_with_deadline(
    path: impl AsRef<Path>,
    deadline: &SamplingDeadline,
) -> Result<BlueprintFile> {
    parquet_blueprint_from_path_metadata(path, deadline)
}

#[cfg(feature = "sampling")]
pub fn parquet_blueprint_from_path_with_options(
    path: impl AsRef<Path>,
    options: &DecodedCompressionOptions,
) -> Result<BlueprintFile> {
    let deadline = options.deadline();
    parquet_blueprint_from_path_with_options_and_deadline(path, options, &deadline)
}

/// Build and sample a Parquet Blueprint under a caller-owned, absolute deadline.
/// The deadline covers metadata, decoded sampling, and final compression.
#[cfg(feature = "sampling")]
pub fn parquet_blueprint_from_path_with_options_and_deadline(
    path: impl AsRef<Path>,
    options: &DecodedCompressionOptions,
    deadline: &SamplingDeadline,
) -> Result<BlueprintFile> {
    let path = path.as_ref();
    let mut blueprint = parquet_blueprint_from_path_metadata(path, deadline)?;
    if options.is_enabled() {
        apply_parquet_decoded_compression(path, &mut blueprint, options, deadline)?;
    }
    Ok(blueprint)
}

fn parquet_blueprint_from_path_metadata(
    path: impl AsRef<Path>,
    deadline: &SamplingDeadline,
) -> Result<BlueprintFile> {
    let path = path.as_ref();
    deadline.check("opening Parquet metadata")?;
    let file = open_preflighted_parquet(path, "for metadata capture")?;
    let reader = SerializedFileReader::new(file)
        .with_context(|| format!("reading Parquet metadata from {}", path.display()))?;
    let metadata = reader.metadata();
    let file_metadata = metadata.file_metadata();
    let schema = file_metadata.schema_descr();
    let row_count = file_metadata.num_rows().max(0) as u64;

    let mut table = BlueprintTable {
        rows: row_count,
        storage_bytes: std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0),
        schema: "parquet".to_string(),
        source_partitions: 1,
        row_group_count: metadata.num_row_groups() as u64,
        ..Default::default()
    };

    let mut table_compressed = 0u64;
    let mut table_uncompressed = 0u64;
    let mut codecs = BTreeSet::new();
    for (idx, descriptor) in schema.columns().iter().enumerate() {
        deadline.check("scanning Parquet column metadata")?;
        let mut column_compressed = 0u64;
        let mut column_uncompressed = 0u64;
        let mut column_values = 0u64;
        let mut column_nulls = 0u64;
        let mut complete_null_statistics = true;
        for row_group_idx in 0..metadata.num_row_groups() {
            deadline.check("scanning Parquet row-group metadata")?;
            let row_group = metadata.row_group(row_group_idx);
            if idx >= row_group.num_columns() {
                continue;
            }
            let column = row_group.column(idx);
            column_compressed =
                column_compressed.saturating_add(column.compressed_size().max(0) as u64);
            column_uncompressed =
                column_uncompressed.saturating_add(column.uncompressed_size().max(0) as u64);
            column_values = column_values.saturating_add(column.num_values().max(0) as u64);
            codecs.insert(format!("{:?}", column.compression()).to_ascii_lowercase());
            match column
                .statistics()
                .and_then(|statistics| statistics.null_count_opt())
            {
                Some(nulls) => column_nulls = column_nulls.saturating_add(nulls),
                None => complete_null_statistics = false,
            }
        }
        table_compressed = table_compressed.saturating_add(column_compressed);
        table_uncompressed = table_uncompressed.saturating_add(column_uncompressed);

        let encoded_len_avg = if column_values > 0 {
            column_uncompressed / column_values
        } else {
            0
        };
        let mut column = parquet_column_blueprint(descriptor);
        column.ordinal = (idx + 1) as u32;
        column.nullable = descriptor.max_def_level() > 0;
        column.null_fraction = if complete_null_statistics && row_count > 0 {
            Some((column_nulls as f64 / row_count as f64).clamp(0.0, 1.0))
        } else {
            None
        };
        column.len_avg = encoded_len_avg;
        // Footer bytes include Parquet encoding overhead and do not provide a
        // value distribution. Do not fabricate p95 from an encoded average.
        column.len_p95 = 0;
        column.length_sample_rows = column_values;
        column.length_sample_method = "parquet-footer-encoded-column-bytes".to_string();
        column.compression = Some(BlueprintCompression {
            measured: column_compressed > 0 || column_uncompressed > 0,
            sample_rows: column_values,
            sample_bytes: column_uncompressed,
            sample_method: "parquet-footer-column-chunks".to_string(),
            ratio_storage: compression_ratio(column_uncompressed, column_compressed),
            sample_encoding: "parquet-column-chunks".to_string(),
            ..Default::default()
        });
        table.cols.insert(format!("col-{}", idx + 1), column);
    }

    table.table_bytes = table_uncompressed;
    table.source_codec = codecs.into_iter().collect::<Vec<_>>().join(",");
    table.compression = Some(BlueprintCompression {
        measured: table_compressed > 0 || table_uncompressed > 0,
        sample_rows: row_count,
        sample_bytes: table_uncompressed,
        sample_method: "parquet-footer".to_string(),
        ratio_storage: compression_ratio(table_uncompressed, table.storage_bytes),
        sample_encoding: "parquet-file".to_string(),
        ..Default::default()
    });

    let mut tables = BTreeMap::new();
    tables.insert(STRUCTURED_TABLE_ID.to_string(), table);
    deadline.check("finishing Parquet metadata")?;
    Ok(BlueprintFile {
        schema_version: SCHEMA_VERSION,
        engine: "parquet".to_string(),
        source_kind: "parquet".to_string(),
        totals: Totals {
            table_count: 1,
            row_count,
            table_bytes: table_uncompressed,
            index_bytes: 0,
        },
        dataset_scope: Some(crate::DatasetScope::structured_dataset(
            "parquet-footer",
            "parquet-footer",
        )),
        tables,
        ..Default::default()
    })
}

#[cfg(feature = "sampling")]
fn apply_parquet_decoded_compression(
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
        .map(|(name, col)| (name.clone(), crate::type_tag_for_column(col)))
        .collect::<Vec<_>>();
    if ordered.is_empty() {
        return Ok(());
    }
    if ordered.iter().any(|(name, _)| {
        table
            .cols
            .get(name)
            .map(|column| !column.source_semantics.is_empty())
            .unwrap_or(false)
    }) {
        anyhow::bail!(
            "decoded Parquet sampling does not flatten nested or repeated values; capture scalar columns separately or omit --measure-compression"
        );
    }

    let file = open_preflighted_parquet(path, "for decoded sampling")?;
    let reader = SerializedFileReader::new(file)
        .with_context(|| format!("reading Parquet rows from {}", path.display()))?;
    let mut row_iter = reader
        .get_row_iter(None)
        .context("creating Parquet row iterator")?;
    let mut acc = CompressionSampleAccumulator::with_max_resident_bytes(
        ordered.len(),
        options.max_sample_bytes,
    )?;

    'rows: for row_result in row_iter.by_ref().take(options.sample_rows as usize) {
        deadline.check("decoding a Parquet sample row")?;
        let row = row_result.context("decoding Parquet sample row")?;
        let fields = row.into_columns();
        let mut remaining_input_bytes = acc.max_input_row_bytes();
        let cell_headers = ordered
            .len()
            .saturating_mul(std::mem::size_of::<OwnedCell>());
        if cell_headers > remaining_input_bytes {
            break;
        }
        remaining_input_bytes -= cell_headers;
        let mut cells = Vec::with_capacity(ordered.len());
        for (idx, (_name, tag)) in ordered.iter().enumerate() {
            let field = fields.get(idx).map(|(_, field)| field);
            let cell = match field {
                Some(field) => {
                    let Some(cell) =
                        parquet_field_to_cell_bounded(field, *tag, remaining_input_bytes)
                    else {
                        break 'rows;
                    };
                    cell
                }
                None => OwnedCell::null(),
            };
            remaining_input_bytes = remaining_input_bytes.saturating_sub(
                cell.bytes
                    .as_ref()
                    .map_or(1, |bytes| bytes.len().saturating_add(1)),
            );
            cells.push(cell);
        }
        if !acc.push_row_bounded(&cells)? {
            break;
        }
    }

    if acc.sample_rows() > 0 {
        table.table_bytes = ((acc.logical_sample_bytes() as u128)
            .saturating_mul(table.rows as u128)
            .saturating_add(acc.sample_rows() as u128 - 1)
            / acc.sample_rows() as u128)
            .min(u64::MAX as u128) as u64;
        blueprint.totals.table_bytes = table.table_bytes;
    }
    let ratio_storage = compression_ratio(table.table_bytes, table.storage_bytes);
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
    deadline.check("computing Parquet sample column statistics")?;
    let column_statistics = acc.column_statistics();
    let column_cardinalities = acc.column_cardinalities(
        table.rows,
        options.column_sample_method.as_str(),
        sampled_with_bias,
        bias_reason,
    );
    for ((((name, _tag), compression), statistics), cardinality) in ordered
        .iter()
        .zip(column_compressions.into_iter())
        .zip(column_statistics.into_iter())
        .zip(column_cardinalities.into_iter())
    {
        deadline.check("applying Parquet sample column statistics")?;
        if let Some(mut compression) = compression {
            if let Some(column) = table.cols.get_mut(name) {
                compression.ratio_storage = column
                    .compression
                    .as_ref()
                    .map(|source| source.ratio_storage)
                    .unwrap_or_default();
                column.compression = Some(compression);
                column.len_avg = statistics.len_avg;
                column.len_p95 = statistics.len_p95;
                column.null_fraction = Some(statistics.null_fraction);
                column.length_sample_rows = statistics.sample_rows;
                column.length_p95_sample_rows = statistics.len_p95_sample_rows;
                column.length_sample_method = options.column_sample_method.clone();
                column.cardinality = cardinality;
            }
        }
    }
    deadline.check("finishing Parquet decoded sampling")?;
    Ok(())
}

#[cfg(feature = "sampling")]
fn parquet_field_to_cell_bounded(
    field: &Field,
    fallback_tag: TypeTag,
    max_payload_bytes: usize,
) -> Option<OwnedCell> {
    let cell = match field {
        Field::Null => return Some(OwnedCell::null()),
        Field::Bool(value) => OwnedCell::new(TypeTag::BoolText, value.to_string().into_bytes()),
        Field::Byte(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::Short(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::Int(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::Long(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::UByte(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::UShort(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::UInt(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::ULong(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::Float16(value) => {
            OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes())
        }
        Field::Float(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::Double(value) => OwnedCell::new(TypeTag::NumberText, value.to_string().into_bytes()),
        Field::Decimal(_) => OwnedCell::new(TypeTag::NumberText, field.to_string().into_bytes()),
        Field::Str(value) => {
            if value.len() > max_payload_bytes {
                return None;
            }
            OwnedCell::new(
                if matches!(fallback_tag, TypeTag::JsonText) {
                    TypeTag::JsonText
                } else {
                    TypeTag::TextUtf8
                },
                value.as_bytes().to_vec(),
            )
        }
        Field::Bytes(value) => {
            if value.data().len() > max_payload_bytes {
                return None;
            }
            if matches!(
                fallback_tag,
                TypeTag::TextUtf8 | TypeTag::JsonText | TypeTag::UnknownText
            ) {
                OwnedCell::new(fallback_tag, value.data().to_vec())
            } else {
                OwnedCell::new(TypeTag::BinaryRaw, value.data().to_vec())
            }
        }
        Field::Date(_) => OwnedCell::new(TypeTag::DateText, field.to_string().into_bytes()),
        Field::TimeMillis(_) | Field::TimeMicros(_) => {
            OwnedCell::new(TypeTag::TimeText, field.to_string().into_bytes())
        }
        Field::TimestampMillis(_) | Field::TimestampMicros(_) => {
            OwnedCell::new(TypeTag::TimestampText, field.to_string().into_bytes())
        }
        Field::Group(_) | Field::ListInternal(_) | Field::MapInternal(_) => {
            OwnedCell::new(TypeTag::JsonText, format!("{field}").into_bytes())
        }
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

fn compression_ratio(uncompressed: u64, compressed: u64) -> f64 {
    if compressed == 0 || uncompressed == 0 {
        0.0
    } else {
        uncompressed as f64 / compressed as f64
    }
}

fn parquet_column_blueprint(descriptor: &ColumnDescriptor) -> BlueprintColumn {
    let mut column = BlueprintColumn {
        column_type: parquet_type_label(descriptor),
        style: parquet_style_label(descriptor),
        native_type: parquet_native_type(descriptor),
        ..Default::default()
    };
    if descriptor.max_rep_level() > 0 {
        column.column_type = "json".to_string();
        column.style = "json".to_string();
        column.source_semantics = "repeated-leaf".to_string();
    } else if descriptor.path().parts().len() > 1 {
        column.column_type = "json".to_string();
        column.style = "json".to_string();
        column.source_semantics = "nested-json".to_string();
    }
    match descriptor.logical_type_ref() {
        Some(LogicalType::Integer {
            bit_width,
            is_signed,
        }) => {
            column.bit_width = (*bit_width).max(0) as u64;
            column.numeric_unsigned = !*is_signed;
        }
        Some(LogicalType::Decimal { scale, precision }) => {
            column.numeric_precision = (*precision).max(0) as u64;
            column.numeric_scale = (*scale).max(0) as u64;
        }
        Some(LogicalType::Time { unit, .. }) | Some(LogicalType::Timestamp { unit, .. }) => {
            column.datetime_precision = parquet_time_precision(unit);
        }
        Some(LogicalType::Uuid) => {
            column.declared_max_chars = 36;
            column.declared_max_bytes = 36;
        }
        _ => {}
    }
    if descriptor.logical_type_ref().is_none() {
        match descriptor.converted_type() {
            ConvertedType::DECIMAL => {
                column.numeric_precision = descriptor.type_precision().max(0) as u64;
                column.numeric_scale = descriptor.type_scale().max(0) as u64;
            }
            ConvertedType::UINT_8 | ConvertedType::INT_8 => {
                column.bit_width = 8;
                column.numeric_unsigned = descriptor.converted_type() == ConvertedType::UINT_8;
            }
            ConvertedType::UINT_16 | ConvertedType::INT_16 => {
                column.bit_width = 16;
                column.numeric_unsigned = descriptor.converted_type() == ConvertedType::UINT_16;
            }
            ConvertedType::UINT_32 | ConvertedType::INT_32 => {
                column.bit_width = 32;
                column.numeric_unsigned = descriptor.converted_type() == ConvertedType::UINT_32;
            }
            ConvertedType::UINT_64 | ConvertedType::INT_64 => {
                column.bit_width = 64;
                column.numeric_unsigned = descriptor.converted_type() == ConvertedType::UINT_64;
            }
            ConvertedType::TIME_MILLIS | ConvertedType::TIMESTAMP_MILLIS => {
                column.datetime_precision = 3;
            }
            ConvertedType::TIME_MICROS | ConvertedType::TIMESTAMP_MICROS => {
                column.datetime_precision = 6;
            }
            _ => {}
        }
    }
    if column.bit_width == 0 {
        column.bit_width = match descriptor.physical_type() {
            parquet::basic::Type::INT32 => 32,
            parquet::basic::Type::INT64 => 64,
            _ => 0,
        };
    }
    if descriptor.type_length() > 0 && column.column_type == "bytes" {
        column.declared_max_bytes = descriptor.type_length() as u64;
    }
    column
}

fn parquet_type_label(descriptor: &ColumnDescriptor) -> String {
    if let Some(logical) = descriptor.logical_type_ref() {
        return match logical {
            LogicalType::String | LogicalType::Enum => "string",
            LogicalType::Json
            | LogicalType::Map
            | LogicalType::List
            | LogicalType::Variant { .. } => "json",
            LogicalType::Date => "date",
            LogicalType::Timestamp { .. } => "timestamp",
            LogicalType::Time { .. } => "time",
            LogicalType::Decimal { .. } => "decimal",
            LogicalType::Uuid => "uuid",
            LogicalType::Integer { bit_width, .. } if *bit_width <= 8 => "tinyint",
            LogicalType::Integer { bit_width, .. } if *bit_width <= 16 => "smallint",
            LogicalType::Integer { bit_width, .. } if *bit_width <= 32 => "int",
            LogicalType::Integer { .. } => "bigint",
            LogicalType::Bson
            | LogicalType::Geometry { .. }
            | LogicalType::Geography { .. }
            | LogicalType::Unknown
            | LogicalType::_Unknown { .. } => "bytes",
            LogicalType::Float16 => "float",
        }
        .to_string();
    }
    let converted = match descriptor.converted_type() {
        ConvertedType::UTF8 | ConvertedType::ENUM => Some("string"),
        ConvertedType::MAP
        | ConvertedType::MAP_KEY_VALUE
        | ConvertedType::LIST
        | ConvertedType::JSON => Some("json"),
        ConvertedType::DECIMAL => Some("decimal"),
        ConvertedType::DATE => Some("date"),
        ConvertedType::TIME_MILLIS | ConvertedType::TIME_MICROS => Some("time"),
        ConvertedType::TIMESTAMP_MILLIS | ConvertedType::TIMESTAMP_MICROS => Some("timestamp"),
        ConvertedType::UINT_8 | ConvertedType::INT_8 => Some("tinyint"),
        ConvertedType::UINT_16 | ConvertedType::INT_16 => Some("smallint"),
        ConvertedType::UINT_32 | ConvertedType::INT_32 => Some("int"),
        ConvertedType::UINT_64 | ConvertedType::INT_64 => Some("bigint"),
        ConvertedType::BSON | ConvertedType::INTERVAL => Some("bytes"),
        ConvertedType::NONE => None,
    };
    if let Some(converted) = converted {
        return converted.to_string();
    }
    match descriptor.physical_type() {
        parquet::basic::Type::BOOLEAN => "boolean",
        parquet::basic::Type::INT32 => "int",
        parquet::basic::Type::INT64 => "bigint",
        parquet::basic::Type::INT96 => "timestamp",
        parquet::basic::Type::FLOAT => "float",
        parquet::basic::Type::DOUBLE => "double",
        parquet::basic::Type::BYTE_ARRAY | parquet::basic::Type::FIXED_LEN_BYTE_ARRAY => "bytes",
    }
    .to_string()
}

fn parquet_style_label(descriptor: &ColumnDescriptor) -> String {
    match descriptor.logical_type_ref() {
        Some(
            LogicalType::Json | LogicalType::Map | LogicalType::List | LogicalType::Variant { .. },
        ) => "json".to_string(),
        Some(LogicalType::String | LogicalType::Enum | LogicalType::Uuid) => "text".to_string(),
        _ => match descriptor.converted_type() {
            ConvertedType::JSON | ConvertedType::MAP | ConvertedType::LIST => "json".to_string(),
            ConvertedType::UTF8 | ConvertedType::ENUM => "text".to_string(),
            _ => String::new(),
        },
    }
}

fn parquet_native_type(descriptor: &ColumnDescriptor) -> String {
    match descriptor.logical_type_ref() {
        Some(LogicalType::Timestamp {
            is_adjusted_to_u_t_c,
            unit,
        }) => format!(
            "parquet:timestamp[unit={},adjusted_to_utc={is_adjusted_to_u_t_c}]",
            parquet_time_unit_label(unit)
        ),
        Some(LogicalType::Time {
            is_adjusted_to_u_t_c,
            unit,
        }) => format!(
            "parquet:time[unit={},adjusted_to_utc={is_adjusted_to_u_t_c}]",
            parquet_time_unit_label(unit)
        ),
        // CRS is producer-supplied free text. Preserve the useful logical
        // family without copying that unbounded source string into the
        // transferable Blueprint.
        Some(LogicalType::Geometry { .. }) => "parquet:geometry".to_string(),
        Some(LogicalType::Geography { .. }) => "parquet:geography".to_string(),
        Some(logical) => format!("parquet:{}", format!("{logical:?}").to_ascii_lowercase()),
        None if descriptor.converted_type() != ConvertedType::NONE => {
            match descriptor.converted_type() {
                ConvertedType::DECIMAL => format!(
                    "parquet:converted-decimal[precision={},scale={}]",
                    descriptor.type_precision(),
                    descriptor.type_scale()
                ),
                converted => format!("parquet:converted-{converted:?}").to_ascii_lowercase(),
            }
        }
        None => format!("parquet:{:?}", descriptor.physical_type()).to_ascii_lowercase(),
    }
}

fn parquet_time_precision(unit: &TimeUnit) -> u64 {
    match unit {
        TimeUnit::MILLIS => 3,
        TimeUnit::MICROS => 6,
        TimeUnit::NANOS => 9,
    }
}

fn parquet_time_unit_label(unit: &TimeUnit) -> &'static str {
    match unit {
        TimeUnit::MILLIS => "millis",
        TimeUnit::MICROS => "micros",
        TimeUnit::NANOS => "nanos",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::basic::{LogicalType, Repetition, TimeUnit, Type as PhysicalType};
    use parquet::column::writer::ColumnWriter;
    use parquet::data_type::{ByteArray, Int64Type};
    use parquet::file::properties::{EnabledStatistics, WriterProperties};
    use parquet::file::writer::SerializedFileWriter;
    use parquet::schema::parser::parse_message_type;
    use parquet::schema::types::{ColumnPath, Type};
    use std::sync::Arc;
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
        dir.join(format!("{name}-{nonce}.parquet"))
    }

    fn encode_compact_varint(mut value: u64, output: &mut Vec<u8>) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            output.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    fn encode_compact_i32(value: i32, output: &mut Vec<u8>) {
        let encoded = ((value as u32) << 1) ^ ((value >> 31) as u32);
        encode_compact_varint(u64::from(encoded), output);
    }

    fn encode_schema_element(name: &str, num_children: Option<i32>, output: &mut Vec<u8>) {
        // SchemaElement field 4: name (binary), encoded with a field-id delta
        // of four from the struct-local initial field id.
        output.push(0x48);
        encode_compact_varint(name.len() as u64, output);
        output.extend_from_slice(name.as_bytes());
        if let Some(num_children) = num_children {
            // Field 5: num_children (i32), delta one from field 4.
            output.push(0x15);
            encode_compact_i32(num_children, output);
        }
        output.push(COMPACT_STOP);
    }

    fn parquet_file_with_schema_depth(group_count: usize) -> Vec<u8> {
        let element_count = group_count + 2; // root, nested groups, leaf
        let mut footer = Vec::new();
        // FileMetaData field 1: version (i32 = 1).
        footer.push(0x15);
        encode_compact_i32(1, &mut footer);
        // Field 2: schema (list<struct>).
        footer.push(0x19);
        footer.push(0xfc); // extended list count, element type struct
        encode_compact_varint(element_count as u64, &mut footer);
        encode_schema_element("root", Some(1), &mut footer);
        for depth in 0..group_count {
            encode_schema_element(&format!("level_{depth}"), Some(1), &mut footer);
        }
        encode_schema_element("leaf", None, &mut footer);
        // Required field 3: num_rows (i64 = 0).
        footer.push(0x16);
        encode_compact_varint(0, &mut footer);
        // Required field 4: row_groups (empty list<struct>).
        footer.push(0x19);
        footer.push(0x0c);
        footer.push(COMPACT_STOP);

        let mut file_bytes = PARQUET_MAGIC.to_vec();
        file_bytes.extend_from_slice(&footer);
        file_bytes.extend_from_slice(&(footer.len() as u32).to_le_bytes());
        file_bytes.extend_from_slice(PARQUET_MAGIC);
        file_bytes
    }

    #[test]
    fn parquet_schema_depth_is_rejected_before_upstream_reader() {
        let path = test_path("deep-schema");
        std::fs::write(
            &path,
            parquet_file_with_schema_depth(MAX_PARQUET_SCHEMA_DEPTH),
        )
        .unwrap();

        let error = parquet_blueprint_from_path(&path).unwrap_err();
        assert!(error.to_string().contains("checking Parquet schema safety"));
        assert!(format!("{error:#}").contains("schema nesting exceeds"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parquet_schema_depth_limit_has_an_exact_boundary() {
        let mut accepted = vec![Some(1); MAX_PARQUET_SCHEMA_DEPTH - 1];
        accepted.push(None);
        validate_parquet_schema_depth(&accepted).unwrap();

        let mut rejected = vec![Some(1); MAX_PARQUET_SCHEMA_DEPTH];
        rejected.push(None);
        assert!(validate_parquet_schema_depth(&rejected).is_err());
    }

    #[test]
    fn type_label_uses_logical_string() {
        let ty = Type::primitive_type_builder("body", PhysicalType::BYTE_ARRAY)
            .with_repetition(Repetition::OPTIONAL)
            .with_logical_type(Some(LogicalType::String))
            .build()
            .unwrap();
        let descriptor = ColumnDescriptor::new(Arc::new(ty), 1, 0, ColumnPath::from("body"));
        assert_eq!(parquet_type_label(&descriptor), "string");
        assert!(descriptor.max_def_level() > 0);
    }

    #[test]
    fn logical_integer_metadata_preserves_signedness_and_width() {
        let unsigned = Type::primitive_type_builder("small_code", PhysicalType::INT32)
            .with_repetition(Repetition::REQUIRED)
            .with_logical_type(Some(LogicalType::Integer {
                bit_width: 16,
                is_signed: false,
            }))
            .build()
            .unwrap();
        let descriptor =
            ColumnDescriptor::new(Arc::new(unsigned), 0, 0, ColumnPath::from("small_code"));
        let column = parquet_column_blueprint(&descriptor);
        assert_eq!(column.column_type, "smallint");
        assert_eq!(column.bit_width, 16);
        assert!(column.numeric_unsigned);

        let signed = Type::primitive_type_builder("signed_code", PhysicalType::INT32)
            .with_repetition(Repetition::REQUIRED)
            .with_logical_type(Some(LogicalType::Integer {
                bit_width: 8,
                is_signed: true,
            }))
            .build()
            .unwrap();
        let descriptor =
            ColumnDescriptor::new(Arc::new(signed), 0, 0, ColumnPath::from("signed_code"));
        let column = parquet_column_blueprint(&descriptor);
        assert_eq!(column.bit_width, 8);
        assert!(!column.numeric_unsigned);
    }

    #[cfg(feature = "sampling")]
    #[test]
    fn oversized_parquet_text_is_rejected_before_sample_copy() {
        let field = Field::Str("界".repeat(1024));
        assert!(parquet_field_to_cell_bounded(&field, TypeTag::TextUtf8, 64).is_none());
    }

    #[test]
    fn logical_metadata_preserves_decimal_and_timestamp_semantics() {
        let decimal = Type::primitive_type_builder("amount", PhysicalType::FIXED_LEN_BYTE_ARRAY)
            .with_repetition(Repetition::REQUIRED)
            .with_length(8)
            .with_precision(18)
            .with_scale(5)
            .with_logical_type(Some(LogicalType::Decimal {
                precision: 18,
                scale: 5,
            }))
            .build()
            .unwrap();
        let descriptor = ColumnDescriptor::new(Arc::new(decimal), 0, 0, ColumnPath::from("amount"));
        let column = parquet_column_blueprint(&descriptor);
        assert_eq!(column.column_type, "decimal");
        assert_eq!(column.numeric_precision, 18);
        assert_eq!(column.numeric_scale, 5);

        let fixed = Type::primitive_type_builder("digest", PhysicalType::FIXED_LEN_BYTE_ARRAY)
            .with_repetition(Repetition::REQUIRED)
            .with_length(8)
            .build()
            .unwrap();
        let descriptor = ColumnDescriptor::new(Arc::new(fixed), 0, 0, ColumnPath::from("digest"));
        let column = parquet_column_blueprint(&descriptor);
        assert_eq!(column.column_type, "bytes");
        assert_eq!(column.declared_max_bytes, 8);

        let timestamp = Type::primitive_type_builder("created", PhysicalType::INT64)
            .with_repetition(Repetition::REQUIRED)
            .with_logical_type(Some(LogicalType::Timestamp {
                is_adjusted_to_u_t_c: true,
                unit: TimeUnit::MICROS,
            }))
            .build()
            .unwrap();
        let descriptor =
            ColumnDescriptor::new(Arc::new(timestamp), 0, 0, ColumnPath::from("created"));
        let column = parquet_column_blueprint(&descriptor);
        assert_eq!(column.datetime_precision, 6);
        assert!(column.native_type.contains("adjusted_to_utc=true"));
    }

    #[test]
    fn nested_leaf_is_explicit_json_without_exposing_its_path() {
        let leaf = Type::primitive_type_builder("secret_name", PhysicalType::BYTE_ARRAY)
            .with_repetition(Repetition::OPTIONAL)
            .with_logical_type(Some(LogicalType::String))
            .build()
            .unwrap();
        let descriptor = ColumnDescriptor::new(
            Arc::new(leaf),
            1,
            0,
            ColumnPath::new(vec![
                "secret_profile".to_string(),
                "secret_name".to_string(),
            ]),
        );
        let column = parquet_column_blueprint(&descriptor);
        assert_eq!(column.column_type, "json");
        assert_eq!(column.style, "json");
        assert_eq!(column.source_semantics, "nested-json");
        assert!(!column.native_type.contains("secret_profile"));
        assert!(!column.native_type.contains("secret_name"));
    }

    #[test]
    fn geospatial_native_types_do_not_expose_producer_crs_text() {
        let secret_crs = "urn:customer:tenant-42:private-projection";
        let geometry = Type::primitive_type_builder("location", PhysicalType::BYTE_ARRAY)
            .with_repetition(Repetition::OPTIONAL)
            .with_logical_type(Some(LogicalType::Geometry {
                crs: Some(secret_crs.to_string()),
            }))
            .build()
            .unwrap();
        let geometry_descriptor =
            ColumnDescriptor::new(Arc::new(geometry), 0, 0, ColumnPath::from("location"));
        assert_eq!(
            parquet_native_type(&geometry_descriptor),
            "parquet:geometry"
        );
        assert!(!parquet_native_type(&geometry_descriptor).contains(secret_crs));

        let geography = Type::primitive_type_builder("region", PhysicalType::BYTE_ARRAY)
            .with_repetition(Repetition::OPTIONAL)
            .with_logical_type(Some(LogicalType::Geography {
                crs: Some(secret_crs.to_string()),
                algorithm: None,
            }))
            .build()
            .unwrap();
        let geography_descriptor =
            ColumnDescriptor::new(Arc::new(geography), 0, 0, ColumnPath::from("region"));
        assert_eq!(
            parquet_native_type(&geography_descriptor),
            "parquet:geography"
        );
        assert!(!parquet_native_type(&geography_descriptor).contains(secret_crs));
    }

    #[test]
    fn parquet_storage_provenance_is_not_transport_compression() {
        let path = test_path("observed");
        let schema = parse_message_type(
            "message test { OPTIONAL BYTE_ARRAY name (UTF8); REQUIRED INT64 id; }",
        )
        .unwrap();
        let properties = WriterProperties::builder()
            .set_statistics_enabled(EnabledStatistics::Chunk)
            .build();
        let file = File::create(&path).unwrap();
        let mut writer =
            SerializedFileWriter::new(file, schema.into(), Arc::new(properties)).unwrap();
        let mut row_group = writer.next_row_group().unwrap();
        let mut name_writer = row_group.next_column().unwrap().unwrap();
        match name_writer.untyped() {
            ColumnWriter::ByteArrayColumnWriter(writer) => {
                let values = [ByteArray::from("alpha"), ByteArray::from("longer-name")];
                writer.write_batch(&values, Some(&[1, 0, 1]), None).unwrap();
            }
            _ => panic!("name must be byte array"),
        }
        name_writer.close().unwrap();
        let mut id_writer = row_group.next_column().unwrap().unwrap();
        id_writer
            .typed::<Int64Type>()
            .write_batch(&[1, 2, 3], None, None)
            .unwrap();
        id_writer.close().unwrap();
        row_group.close().unwrap();
        writer.close().unwrap();

        let expired = SamplingDeadline::after(std::time::Duration::ZERO);
        let error = parquet_blueprint_from_path_with_deadline(&path, &expired)
            .expect_err("caller-owned Parquet deadline must be honored");
        assert!(error.to_string().contains("deadline expired"));

        let metadata_blueprint = parquet_blueprint_from_path(&path).unwrap();
        let metadata_table = metadata_blueprint.tables.get(STRUCTURED_TABLE_ID).unwrap();
        assert_eq!(metadata_table.rows, 3);
        assert_eq!(metadata_table.row_group_count, 1);
        assert!(metadata_table.storage_bytes > 0);
        assert_eq!(metadata_table.cols["col-1"].len_p95, 0);
        assert_eq!(metadata_table.cols["col-1"].null_fraction, Some(1.0 / 3.0));
        let storage_compression = metadata_table.compression.as_ref().unwrap();
        assert_eq!(storage_compression.sample_encoding, "parquet-file");
        assert_eq!(storage_compression.ratio_zstd_3, 0.0);
        assert_eq!(
            storage_compression.ratio_storage,
            metadata_table.table_bytes as f64 / metadata_table.storage_bytes as f64
        );
        let metadata_toml = toml::to_string_pretty(&metadata_blueprint).unwrap();
        assert!(metadata_toml.contains("storage_bytes ="));
        assert!(metadata_toml.contains("row_group_count = 1"));
        assert!(metadata_toml.contains("ratio_storage ="));
        assert!(!metadata_toml.contains("cols.name"));
        assert!(!metadata_toml.contains("cols.id"));
        assert!(
            metadata_toml.contains("type = \"string\""),
            "{metadata_toml}"
        );
        assert!(!metadata_toml.contains("column_type ="));
        assert!(!metadata_toml.contains("ratio_zstd_3 ="));

        #[cfg(feature = "sampling")]
        {
            let shared_deadline = SamplingDeadline::unlimited();
            let sampled_blueprint = parquet_blueprint_from_path_with_options_and_deadline(
                &path,
                &DecodedCompressionOptions::enabled(3, "table-test", "column-test"),
                &shared_deadline,
            )
            .unwrap();
            let sampled_table = sampled_blueprint.tables.get(STRUCTURED_TABLE_ID).unwrap();
            assert!(sampled_table.table_bytes > 0);
            assert_eq!(sampled_table.cols["col-1"].len_avg, 8);
            assert_eq!(sampled_table.cols["col-1"].len_p95, 11);
            assert_eq!(sampled_table.cols["col-1"].null_fraction, Some(1.0 / 3.0));
            assert_eq!(
                sampled_table.compression.as_ref().unwrap().sample_encoding,
                crate::SAMPLE_ENCODING_TAG
            );
            assert!(
                !sampled_table
                    .compression
                    .as_ref()
                    .unwrap()
                    .sampled_with_bias
            );
            assert!(sampled_table
                .compression
                .as_ref()
                .unwrap()
                .bias_reason
                .is_empty());
            assert_eq!(
                sampled_table.compression.as_ref().unwrap().ratio_storage,
                sampled_table.table_bytes as f64 / sampled_table.storage_bytes as f64
            );
            assert!(
                sampled_table.cols["col-1"]
                    .compression
                    .as_ref()
                    .unwrap()
                    .ratio_storage
                    > 0.0
            );
            let sampled_toml = toml::to_string_pretty(&sampled_blueprint).unwrap();
            assert!(sampled_toml.contains("ratio_zstd_3 ="));
            assert!(sampled_toml.contains("ratio_storage ="));
            assert!(sampled_toml.contains("length_sample_method ="));

            let partial_blueprint = parquet_blueprint_from_path_with_options(
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
}
