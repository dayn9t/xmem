//! Error types for xmem

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("shared memory error: {0}")]
    SharedMemory(String),

    #[error("buffer not found: index {0}")]
    BufferNotFound(u32),

    #[error("buffer type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("access denied: buffer is read-only")]
    ReadOnly,

    #[error("buffer already forgotten")]
    AlreadyForgotten,

    #[error("invalid shape: {0}")]
    InvalidShape(String),

    #[cfg(feature = "cuda")]
    #[error("CUDA error: {0}")]
    Cuda(String),
}

pub type Result<T> = std::result::Result<T, Error>;
