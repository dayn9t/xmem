//! xmem - Cross-process shared memory pool

pub mod dtype;
pub mod error;
pub mod storage;

pub use dtype::DType;
pub use error::{Error, Result};
pub use storage::{AccessMode, StorageType};
