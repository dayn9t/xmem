//! # xmem-core
//!
//! 跨进程共享内存池核心库，支持 CPU 共享内存和 CUDA 显存的零拷贝跨进程共享。
//!
//! ## 特性
//!
//! - 零拷贝跨进程共享内存
//! - CPU 共享内存（基于 POSIX shm）
//! - CUDA 显存共享（基于 CUDA IPC）
//! - RAII 自动资源管理
//! - 引用计数追踪
//!
//! ## 快速开始
//!
//! ```rust,no_run
//! use xmem_core::BufferPool;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 创建共享内存池
//! let pool = BufferPool::create("/my_pool")?;
//!
//! // 分配 buffer
//! let mut buf = pool.acquire_cpu(1024)?;
//! buf.as_cpu_slice_mut()?.copy_from_slice(b"hello world");
//!
//! // 将 meta_index 传递给其他进程
//! let idx = buf.meta_index();
//! # Ok(())
//! # }
//! ```
//!
//! ## 架构
//!
//! - [`BufferPool`]: 管理共享内存缓冲池
//! - [`BufferGuard`]: RAII 访问守卫
//! - [`SharedMemory`]: POSIX 共享内存封装
//! - [`BufferMeta`]: 缓冲区元数据
//!
//! ## CUDA 支持
//!
//! 启用 `cuda` feature 以使用 CUDA 显存共享：
//!
//! ```toml
//! [dependencies]
//! xmem-core = { version = "0.1", features = ["cuda"] }
//! ```

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
