//! xmem - Cross-process shared memory pool

pub mod buffer;
#[cfg(feature = "cuda")]
pub mod cuda;
pub mod dtype;
pub mod error;
pub mod guard;
pub mod meta;
pub mod meta_region;
pub mod pool;
pub mod shm;
pub mod storage;

pub use buffer::BufferData;
#[cfg(feature = "cuda")]
pub use cuda::{CudaBuffer, CudaIpcHandle};
pub use dtype::DType;
pub use error::{Error, Result};
pub use guard::BufferGuard;
pub use meta::{BufferMeta, MAX_NDIM};
pub use meta_region::MetaRegion;
pub use pool::BufferPool;
pub use shm::SharedMemory;
pub use storage::{AccessMode, StorageType};
