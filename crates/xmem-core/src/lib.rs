//! xmem - Cross-process shared memory pool

pub mod dtype;
pub mod error;
pub mod meta;
pub mod shm;
pub mod storage;

pub use dtype::DType;
pub use error::{Error, Result};
pub use meta::{BufferMeta, MAX_NDIM};
pub use shm::SharedMemory;
pub use storage::{AccessMode, StorageType};
