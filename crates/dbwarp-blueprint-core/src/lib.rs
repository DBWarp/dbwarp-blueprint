//! DBWarp Blueprint model, validation and structured-file readers.
//!
//! This crate intentionally stays small by default. Heavy structured-file
//! readers and writers are feature-gated so callers that only need the model do
//! not pull Parquet or Avro dependencies.

mod artifact_complexity;
#[cfg(any(feature = "sampling", feature = "avro"))]
mod canonical;
mod deadline;
mod fidelity;
mod format;
mod generation_plan;
mod generator;
mod io;
mod payload_profile;
mod rounding;
#[cfg(feature = "sampling")]
pub mod sample;
mod transfer_probe;

#[cfg(feature = "avro")]
pub mod avro;
#[cfg(feature = "parquet")]
pub mod parquet;

pub use artifact_complexity::*;
#[cfg(any(feature = "sampling", feature = "avro"))]
pub use canonical::*;
pub use deadline::*;
pub use fidelity::*;
pub use format::*;
pub use generation_plan::*;
pub use generator::*;
pub use io::*;
pub use payload_profile::*;
pub use rounding::*;
#[cfg(feature = "sampling")]
pub use sample::*;
pub use transfer_probe::*;
