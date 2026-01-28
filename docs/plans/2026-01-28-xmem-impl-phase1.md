# Phase 1: 项目骨架 + 基础类型

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 创建 Cargo workspace 结构，定义基础类型和错误处理。

---

## Task 1: 初始化 Cargo Workspace

**Files:**
- Create: `Cargo.toml`
- Create: `crates/xmem-core/Cargo.toml`
- Create: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/dayn9t/xmem"
```

**Step 2: 创建 xmem-core Cargo.toml**

```toml
[package]
name = "xmem-core"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Cross-process shared memory pool with CPU and CUDA support"

[features]
default = []
cuda = ["dep:cudarc"]

[dependencies]
shared_memory = "0.12"
cudarc = { version = "0.12", optional = true }
thiserror = "2"
```

**Step 3: 创建 xmem-core/src/lib.rs 占位**

```rust
//! xmem - Cross-process shared memory pool

pub mod error;

pub use error::{Error, Result};
```

**Step 4: 验证编译**

Run: `cargo check`
Expected: 编译失败，缺少 error 模块

**Step 5: Commit**

```bash
git add Cargo.toml crates/
git commit -m "chore: init cargo workspace structure"
```

---

## Task 2: 定义错误类型

**Files:**
- Create: `crates/xmem-core/src/error.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 error.rs**

```rust
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
```

**Step 2: 验证编译**

Run: `cargo check`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-core/src/error.rs
git commit -m "feat(core): add error types"
```

---

## Task 3: 定义数据类型枚举

**Files:**
- Create: `crates/xmem-core/src/dtype.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 dtype.rs**

```rust
//! Data type definitions

/// Supported data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DType {
    UInt8 = 0,
    Int8 = 1,
    UInt16 = 2,
    Int16 = 3,
    UInt32 = 4,
    Int32 = 5,
    UInt64 = 6,
    Int64 = 7,
    Float16 = 8,
    Float32 = 9,
    Float64 = 10,
}

impl DType {
    /// Size in bytes
    pub const fn size(&self) -> usize {
        match self {
            DType::UInt8 | DType::Int8 => 1,
            DType::UInt16 | DType::Int16 | DType::Float16 => 2,
            DType::UInt32 | DType::Int32 | DType::Float32 => 4,
            DType::UInt64 | DType::Int64 | DType::Float64 => 8,
        }
    }

    /// Convert from u8
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(DType::UInt8),
            1 => Some(DType::Int8),
            2 => Some(DType::UInt16),
            3 => Some(DType::Int16),
            4 => Some(DType::UInt32),
            5 => Some(DType::Int32),
            6 => Some(DType::UInt64),
            7 => Some(DType::Int64),
            8 => Some(DType::Float16),
            9 => Some(DType::Float32),
            10 => Some(DType::Float64),
            _ => None,
        }
    }
}
```

**Step 2: 更新 lib.rs**

```rust
//! xmem - Cross-process shared memory pool

pub mod dtype;
pub mod error;

pub use dtype::DType;
pub use error::{Error, Result};
```

**Step 3: 验证编译**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/dtype.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add DType enum"
```

---

## Task 4: 定义存储类型枚举

**Files:**
- Create: `crates/xmem-core/src/storage.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 storage.rs**

```rust
//! Storage type definitions

/// Storage location type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StorageType {
    Cpu = 0,
    #[cfg(feature = "cuda")]
    Cuda = 1,
}

impl StorageType {
    /// Convert from u8
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(StorageType::Cpu),
            #[cfg(feature = "cuda")]
            1 => Some(StorageType::Cuda),
            _ => None,
        }
    }
}

/// Access mode for buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}
```

**Step 2: 更新 lib.rs**

```rust
//! xmem - Cross-process shared memory pool

pub mod dtype;
pub mod error;
pub mod storage;

pub use dtype::DType;
pub use error::{Error, Result};
pub use storage::{AccessMode, StorageType};
```

**Step 3: 验证编译**

Run: `cargo check && cargo check --features cuda`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/storage.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add StorageType and AccessMode"
```

---

## Task 5: 定义 BufferMeta 结构

**Files:**
- Create: `crates/xmem-core/src/meta.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 meta.rs**

```rust
//! Buffer metadata structure

use std::sync::atomic::{AtomicI32, AtomicU8, AtomicU32, AtomicU64};

/// Maximum number of dimensions
pub const MAX_NDIM: usize = 8;

/// CUDA IPC handle size (预留，即使 CPU-only 也保留以保证跨进程兼容性)
pub const CUDA_IPC_HANDLE_SIZE: usize = 64;

/// Buffer metadata stored in shared memory
///
/// 所有字段使用原子类型，支持跨进程并发访问
#[repr(C)]
pub struct BufferMeta {
    /// Unique buffer ID
    pub id: AtomicU32,
    /// Reference count (atomic)
    pub ref_count: AtomicI32,
    /// Storage type: 0=cpu, 1=cuda
    pub storage_type: AtomicU8,
    /// GPU device ID (for CUDA)
    pub device_id: AtomicU8,
    /// Data type
    pub dtype: AtomicU8,
    /// Number of dimensions
    pub ndim: AtomicU8,
    /// Shape (up to 8 dimensions)
    pub shape: [AtomicU64; MAX_NDIM],
    /// Strides in bytes
    pub strides: [AtomicU64; MAX_NDIM],
    /// Total size in bytes
    pub size: AtomicU64,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: AtomicU64,
    /// Sequence number
    pub seq: AtomicU64,
    /// Content type string (null-terminated, 不需要原子操作)
    pub content_type: [u8; 32],
    /// Producer name (null-terminated, 不需要原子操作)
    pub producer: [u8; 32],
    /// CUDA IPC handle (预留，仅 storage_type == 1 时有效)
    pub cuda_ipc_handle: [u8; CUDA_IPC_HANDLE_SIZE],
    /// Reserved for future use
    pub reserved: [u8; 64],
}

impl BufferMeta {
    /// Size of BufferMeta in bytes
    pub const SIZE: usize = std::mem::size_of::<Self>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_size() {
        // Ensure struct size is stable for cross-process compatibility
        assert!(BufferMeta::SIZE > 0);
        println!("BufferMeta size: {} bytes", BufferMeta::SIZE);
    }
}
```

**Step 2: 更新 lib.rs**

```rust
//! xmem - Cross-process shared memory pool

pub mod dtype;
pub mod error;
pub mod meta;
pub mod storage;

pub use dtype::DType;
pub use error::{Error, Result};
pub use meta::{BufferMeta, MAX_NDIM};
pub use storage::{AccessMode, StorageType};
```

**Step 3: 运行测试**

Run: `cargo test`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/meta.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add BufferMeta structure"
```

---

## Task 6: 添加 .gitignore

**Files:**
- Create: `.gitignore`

**Step 1: 创建 .gitignore**

```
/target
Cargo.lock
*.so
*.dylib
*.dll
__pycache__/
*.pyc
.pytest_cache/
*.egg-info/
dist/
build/
.venv/
```

**Step 2: Commit**

```bash
git add .gitignore
git commit -m "chore: add .gitignore"
```

---

## Phase 1 完成检查

Run: `cargo test`
Expected: 所有测试通过

**产出文件：**
```
xmem/
├── .gitignore
├── Cargo.toml
└── crates/
    └── xmem-core/
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── error.rs
            ├── dtype.rs
            ├── storage.rs
            └── meta.rs
```
