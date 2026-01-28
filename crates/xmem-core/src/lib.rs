//! xmem - Cross-process shared memory pool

pub mod buffer;
pub mod dtype;
pub mod error;
pub mod guard;
pub mod meta;
pub mod meta_region;
pub mod pool;
pub mod shm;
pub mod storage;

pub use buffer::BufferData;
pub use dtype::DType;
pub use error::{Error, Result};
pub use guard::BufferGuard;
pub use meta::{BufferMeta, MAX_NDIM};
pub use meta_region::MetaRegion;
pub use pool::BufferPool;
pub use shm::SharedMemory;
pub use storage::{AccessMode, StorageType};
