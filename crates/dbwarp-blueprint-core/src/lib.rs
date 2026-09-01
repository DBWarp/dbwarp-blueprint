//! Shared DBWarp Blueprint model and deterministic synthetic data primitives.
//!
//! This crate intentionally stays small by default. Heavy structured-file
//! readers/writers are feature-gated so runtime users can share the same Blueprint
//! vocabulary without always pulling Parquet/Avro dependencies.

#[cfg(any(feature = "sampling", feature = "avro"))]
mod canonical;
mod deadline;
mod fidelity;
mod format;
mod generation_plan;
mod generator;
mod io;
mod rounding;
#[cfg(feature = "sampling")]
pub mod sample;

#[cfg(feature = "avro")]
pub mod avro;
#[cfg(feature = "parquet")]
pub mod parquet;

#[cfg(any(feature = "sampling", feature = "avro"))]
pub use canonical::*;
pub use deadline::*;
pub use fidelity::*;
pub use format::*;
pub use generation_plan::*;
pub use generator::*;
pub use io::*;
pub use rounding::*;
#[cfg(feature = "sampling")]
pub use sample::*;
