//! Bounded local zstd workers for database sampling.
//!
//! Blueprint emits compression ratios, not compressed bytes. Measurements are
//! independent one-shot zstd frames on long-lived contexts: pledging the input
//! size lets zstd size its workspace to the actual sample instead of a
//! worst-case streaming window, which keeps per-call cost flat on allocators
//! that return large freed blocks to the operating system (Windows). Database
//! reads remain sequential; only already-encoded, in-memory samples cross this
//! worker pool.

use std::io;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

use zstd::bulk::Compressor;

use dbwarp_blueprint_core::{
    encode_columnar_transfer_probe_streaming_frames, measure_zstd_streaming_probe_frames,
    measure_zstd_streaming_probe_frames_with_chunk_bytes,
    TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG, TRANSFER_SAMPLE_STREAMING_ENCODING_TAG,
};

use crate::format::{self, BlueprintCompression};
use crate::sample_encode;

#[derive(Debug)]
pub struct PreparedCompressionSample {
    pub column_bytes: Vec<Vec<u8>>,
    pub sample_rows: u64,
    pub sample_method: String,
    pub sampled_with_bias: bool,
    pub bias_reason: String,
    /// When present, replay the persistent zstd context using this probe
    /// chunk boundary and emit the chunk-aware v3 measurement tag.
    pub compression_chunk_bytes: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct CompressionWorkReport {
    pub chunk_level_3_attempts: u64,
    pub table_level_3_attempts: u64,
    pub column_level_3_attempts: u64,
    /// Aggregate wall time spent inside worker compression operations. This
    /// can exceed pipeline wall time when multiple workers overlap.
    pub compression_ms: u64,
}

#[derive(Debug)]
pub struct CompressionMeasurements {
    pub table: BlueprintCompression,
    pub columns: Vec<Option<BlueprintCompression>>,
    pub work: CompressionWorkReport,
}

struct CompressionJob {
    sample: PreparedCompressionSample,
    result_tx: mpsc::SyncSender<io::Result<CompressionMeasurements>>,
}

/// Result handle returned in submission order. Resolving tickets in that same
/// order makes Blueprint output deterministic even when workers finish out of
/// order.
pub struct CompressionTicket {
    result_rx: mpsc::Receiver<io::Result<CompressionMeasurements>>,
}

impl CompressionTicket {
    pub fn resolve(self) -> io::Result<CompressionMeasurements> {
        self.result_rx.recv().map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "compression worker stopped before returning a result",
            )
        })?
    }
}

/// Fixed-size CPU worker pool with a bounded input queue. Each worker owns its
/// zstd contexts, so compression never contends on a shared zstd lock. The
/// queue capacity equals the worker count, bounding retained sample bytes
/// while the single database connection continues its sequential reads.
pub struct CompressionWorkerPool {
    job_tx: Option<mpsc::SyncSender<CompressionJob>>,
    handles: Vec<thread::JoinHandle<()>>,
    worker_count: usize,
    queue_capacity: usize,
}

impl CompressionWorkerPool {
    pub fn new(worker_count: usize) -> io::Result<Self> {
        if worker_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "compression worker count must be at least one",
            ));
        }
        let queue_capacity = worker_count;
        let (job_tx, job_rx) = mpsc::sync_channel::<CompressionJob>(queue_capacity);
        let job_rx = Arc::new(Mutex::new(job_rx));
        let mut handles = Vec::with_capacity(worker_count);

        for worker_index in 0..worker_count {
            let worker_rx = Arc::clone(&job_rx);
            let handle = thread::Builder::new()
                .name(format!("blueprint-zstd-{worker_index}"))
                .spawn(move || {
                    let mut compressors = match ReusableZstdCompressors::new() {
                        Ok(compressors) => compressors,
                        Err(error) => {
                            while let Ok(job) = receive_job(&worker_rx) {
                                let _ = job.result_tx.send(Err(io::Error::new(
                                    error.kind(),
                                    format!("initializing reusable zstd contexts: {error}"),
                                )));
                            }
                            return;
                        }
                    };

                    while let Ok(job) = receive_job(&worker_rx) {
                        let result = analyze_sample(job.sample, &mut compressors);
                        let _ = job.result_tx.send(result);
                    }
                })?;
            handles.push(handle);
        }

        Ok(Self {
            job_tx: Some(job_tx),
            handles,
            worker_count,
            queue_capacity,
        })
    }

    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    pub fn submit(&self, sample: PreparedCompressionSample) -> io::Result<CompressionTicket> {
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        self.job_tx
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "compression pool closed"))?
            .send(CompressionJob { sample, result_tx })
            .map_err(|_| {
                io::Error::new(io::ErrorKind::BrokenPipe, "compression workers stopped")
            })?;
        Ok(CompressionTicket { result_rx })
    }
}

impl Drop for CompressionWorkerPool {
    fn drop(&mut self) {
        // Closing the sender lets workers finish the bounded queue and exit.
        // Join here so no sampled bytes outlive the capture that owns them.
        self.job_tx.take();
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

fn receive_job(
    receiver: &Arc<Mutex<mpsc::Receiver<CompressionJob>>>,
) -> Result<CompressionJob, mpsc::RecvError> {
    match receiver.lock() {
        Ok(receiver) => receiver.recv(),
        Err(_) => Err(mpsc::RecvError),
    }
}

fn analyze_sample(
    sample: PreparedCompressionSample,
    compressors: &mut ReusableZstdCompressors,
) -> io::Result<CompressionMeasurements> {
    if sample.sample_rows == 0 || sample.column_bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "compression sample has no rows or columns",
        ));
    }

    let transfer_frames = encode_columnar_transfer_probe_streaming_frames(
        sample.column_bytes.as_slice(),
        sample.sample_rows,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut work = CompressionWorkReport::default();
    let started = Instant::now();
    let streaming = match sample.compression_chunk_bytes {
        Some(chunk_bytes) => {
            measure_zstd_streaming_probe_frames_with_chunk_bytes(&transfer_frames, 3, chunk_bytes)
        }
        None => measure_zstd_streaming_probe_frames(&transfer_frames, 3),
    }
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
    let compression_attempts = sample.compression_chunk_bytes.map_or_else(
        || transfer_frames.len() as u64,
        |chunk_bytes| {
            transfer_frames
                .iter()
                .map(|frame| frame.len().div_ceil(chunk_bytes) as u64)
                .sum()
        },
    );
    work.chunk_level_3_attempts = work
        .chunk_level_3_attempts
        .saturating_add(compression_attempts);
    work.table_level_3_attempts = work.table_level_3_attempts.saturating_add(1);
    work.compression_ms = work.compression_ms.saturating_add(elapsed_ms(started));
    let streaming = streaming?;
    let comp_3_len = streaming.total_compressed_bytes;
    if comp_3_len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zstd returned an empty table sample",
        ));
    }

    let per_chunk_ratios = transfer_frames
        .iter()
        .zip(streaming.frame_compressed_bytes.iter().copied())
        .filter_map(|(frame, compressed_len)| {
            (compressed_len > 0 && !frame.is_empty())
                .then_some(frame.len() as f64 / compressed_len as f64)
        })
        .collect::<Vec<_>>();

    let stddev = if per_chunk_ratios.len() > 1 {
        let mean = per_chunk_ratios.iter().sum::<f64>() / per_chunk_ratios.len() as f64;
        let variance = per_chunk_ratios
            .iter()
            .map(|ratio| (ratio - mean).powi(2))
            .sum::<f64>()
            / per_chunk_ratios.len() as f64;
        variance.sqrt()
    } else {
        0.0
    };

    let table = BlueprintCompression {
        measured: true,
        sample_rows: sample.sample_rows,
        sample_bytes: format::round_sample_bytes(streaming.total_input_bytes as u64),
        sample_method: sample.sample_method.clone(),
        sampled_with_bias: sample.sampled_with_bias,
        bias_reason: sample.bias_reason.clone(),
        ratio_zstd_3: format::round_ratio(streaming.total_input_bytes as f64 / comp_3_len as f64),
        ratio_stddev: format::round_ratio(stddev),
        sample_encoding: if sample.compression_chunk_bytes.is_some() {
            TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG.to_string()
        } else {
            TRANSFER_SAMPLE_STREAMING_ENCODING_TAG.to_string()
        },
        ..BlueprintCompression::default()
    };

    let mut columns = Vec::with_capacity(sample.column_bytes.len());
    for column in sample.column_bytes {
        if column.is_empty() {
            columns.push(None);
            continue;
        }
        let started = Instant::now();
        let column_3_len = compressors.level_3_len(&column);
        work.column_level_3_attempts = work.column_level_3_attempts.saturating_add(1);
        work.compression_ms = work.compression_ms.saturating_add(elapsed_ms(started));
        let column_3_len = column_3_len?;
        if column_3_len == 0 {
            columns.push(None);
            continue;
        }
        columns.push(Some(BlueprintCompression {
            measured: true,
            sample_rows: sample.sample_rows,
            sample_bytes: format::round_sample_bytes(column.len() as u64),
            sample_method: sample.sample_method.clone(),
            sampled_with_bias: sample.sampled_with_bias,
            bias_reason: sample.bias_reason.clone(),
            ratio_zstd_3: format::round_ratio(column.len() as f64 / column_3_len as f64),
            ratio_stddev: 0.0,
            sample_encoding: sample_encode::SAMPLE_ENCODING_TAG.to_string(),
            ..BlueprintCompression::default()
        }));
    }

    Ok(CompressionMeasurements {
        table,
        columns,
        work,
    })
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

/// One long-lived one-shot zstd context per level. Every measurement is an
/// independent `ZSTD_compress2` frame: the pledged input size selects
/// right-sized parameters and workspace, the context is reused without an
/// explicit session reset, and no history crosses measurements.
pub struct ReusableZstdCompressors {
    level_3: Compressor<'static>,
    output: Vec<u8>,
}

impl ReusableZstdCompressors {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            level_3: Compressor::new(3)?,
            output: Vec::new(),
        })
    }

    pub fn level_3_len(&mut self, input: &[u8]) -> io::Result<usize> {
        compress_len_one_shot(&mut self.level_3, &mut self.output, input)
    }
}

fn compress_len_one_shot(
    compressor: &mut Compressor<'static>,
    output: &mut Vec<u8>,
    input: &[u8],
) -> io::Result<usize> {
    let minimum_output = zstd::zstd_safe::compress_bound(input.len()).max(1);
    if output.len() < minimum_output {
        output.resize(minimum_output, 0);
    }
    compressor.compress_to_buffer(input, output.as_mut_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reused_one_shot_context_matches_independent_frames() {
        let mut compressors = ReusableZstdCompressors::new().unwrap();
        let binary = (0_u32..65_537)
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let repetitive = "blueprint-compression-probe-v2|".repeat(16_384);
        let inputs = [
            &[][..],
            b"one small sampled row".as_slice(),
            repetitive.as_bytes(),
            binary.as_slice(),
        ];

        for _ in 0..2 {
            for input in inputs {
                let expected_3 = zstd::bulk::compress(input, 3).unwrap();
                assert_eq!(compressors.level_3_len(input).unwrap(), expected_3.len());
            }
        }
    }

    fn prepared_sample() -> PreparedCompressionSample {
        let mut column_bytes = vec![Vec::new(), Vec::new()];
        for number in [b"1".as_slice(), b"2".as_slice(), b"3".as_slice()] {
            sample_encode::encode_row(
                &mut column_bytes[0],
                &[sample_encode::Cell::new(
                    sample_encode::TypeTag::TextUtf8,
                    b"alpha",
                )],
            )
            .unwrap();
            sample_encode::encode_row(
                &mut column_bytes[1],
                &[sample_encode::Cell::new(
                    sample_encode::TypeTag::NumberText,
                    number,
                )],
            )
            .unwrap();
        }
        PreparedCompressionSample {
            column_bytes,
            sample_rows: 3,
            sample_method: "deterministic test".to_string(),
            sampled_with_bias: false,
            bias_reason: String::new(),
            compression_chunk_bytes: None,
        }
    }

    fn measurement_signature(measurements: CompressionMeasurements) -> String {
        format!(
            "{:?}|{:?}|{}|{}|{}",
            measurements.table,
            measurements.columns,
            measurements.work.chunk_level_3_attempts,
            measurements.work.table_level_3_attempts,
            measurements.work.column_level_3_attempts,
        )
    }

    #[test]
    fn bounded_workers_preserve_deterministic_v6_measurements() {
        let one = CompressionWorkerPool::new(1).unwrap();
        let one_signature =
            measurement_signature(one.submit(prepared_sample()).unwrap().resolve().unwrap());
        drop(one);

        assert!(one_signature.contains(TRANSFER_SAMPLE_STREAMING_ENCODING_TAG));

        for worker_count in [2, 4, 8] {
            let pool = CompressionWorkerPool::new(worker_count).unwrap();
            let tickets = (0..16)
                .map(|_| pool.submit(prepared_sample()).unwrap())
                .collect::<Vec<_>>();
            for ticket in tickets {
                assert_eq!(
                    measurement_signature(ticket.resolve().unwrap()),
                    one_signature
                );
            }
        }
    }

    #[test]
    fn probe_chunked_samples_emit_distinct_v3_contract() {
        let mut sample = prepared_sample();
        sample.compression_chunk_bytes = Some(8);
        let pool = CompressionWorkerPool::new(1).unwrap();
        let measurement = pool.submit(sample).unwrap().resolve().unwrap();
        assert_eq!(
            measurement.table.sample_encoding,
            TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG
        );
        assert!(measurement.work.chunk_level_3_attempts > 1);
    }

    #[test]
    fn worker_pool_rejects_zero_workers() {
        assert!(CompressionWorkerPool::new(0).is_err());
    }
}
