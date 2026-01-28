# Phase 3: BufferPool + RAII

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 BufferPool 核心功能和 BufferGuard RAII 封装。

**依赖:** Phase 2 完成

---

## Task 1: Buffer 结构定义

**Files:**
- Create: `crates/xmem-core/src/buffer.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 buffer.rs**

```rust
//! Buffer handle and storage

use crate::shm::SharedMemory;
use crate::storage::StorageType;

#[cfg(feature = "cuda")]
use crate::cuda::CudaBuffer;

/// Buffer data storage
pub enum BufferData {
    Cpu(SharedMemory),
    #[cfg(feature = "cuda")]
    Cuda(CudaBuffer),
}

impl BufferData {
    /// Get storage type
    pub fn storage_type(&self) -> StorageType {
        match self {
            BufferData::Cpu(_) => StorageType::Cpu,
            #[cfg(feature = "cuda")]
            BufferData::Cuda(_) => StorageType::Cuda,
        }
    }

    /// Get size in bytes
    pub fn size(&self) -> usize {
        match self {
            BufferData::Cpu(shm) => shm.size(),
            #[cfg(feature = "cuda")]
            BufferData::Cuda(buf) => buf.size(),
        }
    }

    /// Get CPU pointer (only for CPU buffers)
    pub fn as_cpu_ptr(&self) -> Option<*const u8> {
        match self {
            BufferData::Cpu(shm) => Some(shm.as_ptr()),
            #[cfg(feature = "cuda")]
            BufferData::Cuda(_) => None,
        }
    }

    /// Get mutable CPU pointer (only for CPU buffers)
    pub fn as_cpu_mut_ptr(&mut self) -> Option<*mut u8> {
        match self {
            BufferData::Cpu(shm) => Some(shm.as_mut_ptr()),
            #[cfg(feature = "cuda")]
            BufferData::Cuda(_) => None,
        }
    }

    /// Get CUDA device pointer (only for CUDA buffers)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr(&self) -> Option<u64> {
        match self {
            BufferData::Cpu(_) => None,
            BufferData::Cuda(buf) => Some(buf.device_ptr()),
        }
    }
}
```

**Step 2: 更新 lib.rs**

```rust
//! xmem - Cross-process shared memory pool

pub mod buffer;
pub mod dtype;
pub mod error;
pub mod meta;
pub mod meta_region;
pub mod shm;
pub mod storage;

pub use buffer::BufferData;
pub use dtype::DType;
pub use error::{Error, Result};
pub use meta::{BufferMeta, MAX_NDIM};
pub use meta_region::MetaRegion;
pub use shm::SharedMemory;
pub use storage::{AccessMode, StorageType};
```

**Step 3: 验证编译**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/buffer.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add BufferData enum"
```

---

## Task 2: BufferGuard RAII 封装

**Files:**
- Create: `crates/xmem-core/src/guard.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 guard.rs**

```rust
//! RAII buffer guard

use crate::buffer::BufferData;
use crate::storage::AccessMode;
use crate::{Error, Result};
use std::sync::atomic::{AtomicI32, Ordering};

/// RAII guard for buffer access
pub struct BufferGuard {
    /// Buffer data
    data: Option<BufferData>,
    /// Metadata index
    meta_index: u32,
    /// Access mode
    mode: AccessMode,
    /// Reference to ref_count in shared memory
    ref_count: *const AtomicI32,
    /// Whether this guard owns the release responsibility
    should_release: bool,
}

// Safety: BufferGuard can be sent between threads
// The underlying shared memory is process-wide accessible
unsafe impl Send for BufferGuard {}

impl BufferGuard {
    /// Create a new buffer guard
    pub(crate) fn new(
        data: BufferData,
        meta_index: u32,
        mode: AccessMode,
        ref_count: *const AtomicI32,
    ) -> Self {
        Self {
            data: Some(data),
            meta_index,
            mode,
            ref_count,
            should_release: true,
        }
    }

    /// Get metadata index
    pub fn meta_index(&self) -> u32 {
        self.meta_index
    }

    /// Get access mode
    pub fn mode(&self) -> AccessMode {
        self.mode
    }

    /// Check if buffer is still valid
    pub fn is_valid(&self) -> bool {
        self.data.is_some()
    }

    /// Get CPU slice (read-only)
    pub fn as_cpu_slice(&self) -> Result<&[u8]> {
        let data = self.data.as_ref().ok_or(Error::AlreadyForgotten)?;
        let ptr = data.as_cpu_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cpu".to_string(),
            actual: "Cuda".to_string(),
        })?;
        Ok(unsafe { std::slice::from_raw_parts(ptr, data.size()) })
    }

    /// Get CPU slice (mutable, requires ReadWrite mode)
    pub fn as_cpu_slice_mut(&mut self) -> Result<&mut [u8]> {
        if self.mode == AccessMode::ReadOnly {
            return Err(Error::ReadOnly);
        }
        let data = self.data.as_mut().ok_or(Error::AlreadyForgotten)?;
        let size = data.size();
        let ptr = data.as_cpu_mut_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cpu".to_string(),
            actual: "Cuda".to_string(),
        })?;
        Ok(unsafe { std::slice::from_raw_parts_mut(ptr, size) })
    }

    /// Get CUDA device pointer (read-only)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr(&self) -> Result<u64> {
        let data = self.data.as_ref().ok_or(Error::AlreadyForgotten)?;
        data.as_cuda_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cuda".to_string(),
            actual: "Cpu".to_string(),
        })
    }

    /// Get CUDA device pointer (mutable, requires ReadWrite mode)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr_mut(&mut self) -> Result<u64> {
        if self.mode == AccessMode::ReadOnly {
            return Err(Error::ReadOnly);
        }
        self.as_cuda_ptr()
    }

    /// Forget this guard without releasing the buffer
    /// Used when transferring ownership to another process
    pub fn forget(mut self) {
        self.should_release = false;
        self.data = None;
    }

    /// Manually release and decrement ref count
    fn release(&self) {
        if !self.ref_count.is_null() {
            let ref_count = unsafe { &*self.ref_count };
            ref_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for BufferGuard {
    fn drop(&mut self) {
        if self.should_release && self.data.is_some() {
            self.release();
        }
    }
}
```

**Step 2: 更新 lib.rs**

```rust
//! xmem - Cross-process shared memory pool

pub mod buffer;
pub mod dtype;
pub mod error;
pub mod guard;
pub mod meta;
pub mod meta_region;
pub mod shm;
pub mod storage;

pub use buffer::BufferData;
pub use dtype::DType;
pub use error::{Error, Result};
pub use guard::BufferGuard;
pub use meta::{BufferMeta, MAX_NDIM};
pub use meta_region::MetaRegion;
pub use shm::SharedMemory;
pub use storage::{AccessMode, StorageType};
```

**Step 3: 验证编译**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/guard.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add BufferGuard RAII wrapper"
```

---

## Task 3: BufferPool 基础结构

**Files:**
- Create: `crates/xmem-core/src/pool.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 pool.rs**

```rust
//! Buffer pool management

use crate::buffer::BufferData;
use crate::guard::BufferGuard;
use crate::meta::BufferMeta;
use crate::meta_region::MetaRegion;
use crate::shm::SharedMemory;
use crate::storage::{AccessMode, StorageType};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};

/// Default metadata region capacity
const DEFAULT_CAPACITY: usize = 1024;

/// Buffer pool for managing shared memory buffers
pub struct BufferPool {
    /// Pool name
    name: String,
    /// Metadata region
    meta_region: MetaRegion,
    /// Cached buffer data (meta_index -> BufferData)
    buffers: RwLock<HashMap<u32, Arc<BufferData>>>,
}

impl BufferPool {
    /// Create a new buffer pool
    pub fn create(name: &str) -> Result<Self> {
        Self::create_with_capacity(name, DEFAULT_CAPACITY)
    }

    /// Create a new buffer pool with specified capacity
    pub fn create_with_capacity(name: &str, capacity: usize) -> Result<Self> {
        let meta_name = format!("{}_meta", name);
        let meta_region = MetaRegion::create(&meta_name, capacity)?;

        Ok(Self {
            name: name.to_string(),
            meta_region,
            buffers: RwLock::new(HashMap::new()),
        })
    }

    /// Open an existing buffer pool
    pub fn open(name: &str) -> Result<Self> {
        let meta_name = format!("{}_meta", name);
        let meta_region = MetaRegion::open(&meta_name)?;

        Ok(Self {
            name: name.to_string(),
            meta_region,
            buffers: RwLock::new(HashMap::new()),
        })
    }

    /// Get pool name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.meta_region.capacity()
    }

    /// Generate buffer shm name
    fn buffer_shm_name(&self, meta_index: u32) -> String {
        format!("{}_buf_{}", self.name, meta_index)
    }

    /// Acquire a new CPU buffer
    pub fn acquire_cpu(&self, size: usize) -> Result<BufferGuard> {
        // Allocate metadata slot
        let meta_index = self.meta_region.alloc()?;

        // Create shared memory for buffer data
        let shm_name = self.buffer_shm_name(meta_index);
        let shm = SharedMemory::create(&shm_name, size)?;

        // Initialize metadata
        // Safety: we just allocated this slot, no one else has access yet
        let meta_region_ptr = &self.meta_region as *const MetaRegion as *mut MetaRegion;
        let meta = unsafe { (*meta_region_ptr).get_mut(meta_index)? };
        meta.id = meta_index;
        meta.ref_count.store(1, Ordering::SeqCst);
        meta.storage_type = StorageType::Cpu as u8;
        meta.device_id = 0;
        meta.size = size as u64;

        let ref_count_ptr = &meta.ref_count as *const _;

        // Create buffer data
        let data = BufferData::Cpu(shm);

        // Cache it
        {
            let mut buffers = self.buffers.write().unwrap();
            buffers.insert(meta_index, Arc::new(data));
        }

        // Get the cached data for the guard
        let buffers = self.buffers.read().unwrap();
        let data_arc = buffers.get(&meta_index).unwrap().clone();

        // Create a new BufferData for the guard (we need ownership)
        let shm_name = self.buffer_shm_name(meta_index);
        let shm = SharedMemory::open(&shm_name)?;
        let data = BufferData::Cpu(shm);

        Ok(BufferGuard::new(
            data,
            meta_index,
            AccessMode::ReadWrite,
            ref_count_ptr,
        ))
    }

    /// Get an existing buffer (read-only)
    pub fn get(&self, meta_index: u32) -> Result<BufferGuard> {
        self.get_with_mode(meta_index, AccessMode::ReadOnly)
    }

    /// Get an existing buffer (read-write)
    pub fn get_mut(&self, meta_index: u32) -> Result<BufferGuard> {
        self.get_with_mode(meta_index, AccessMode::ReadWrite)
    }

    fn get_with_mode(&self, meta_index: u32, mode: AccessMode) -> Result<BufferGuard> {
        // Get metadata
        let meta = self.meta_region.get(meta_index)?;
        let ref_count_ptr = &meta.ref_count as *const _;

        // Increment ref count
        meta.ref_count.fetch_add(1, Ordering::SeqCst);

        // Open buffer data based on storage type
        let storage_type = StorageType::from_u8(meta.storage_type)
            .ok_or_else(|| Error::SharedMemory("invalid storage type".to_string()))?;

        let data = match storage_type {
            StorageType::Cpu => {
                let shm_name = self.buffer_shm_name(meta_index);
                let shm = SharedMemory::open(&shm_name)?;
                BufferData::Cpu(shm)
            }
            #[cfg(feature = "cuda")]
            StorageType::Cuda => {
                // TODO: implement CUDA buffer opening
                return Err(Error::SharedMemory("CUDA not implemented yet".to_string()));
            }
        };

        Ok(BufferGuard::new(data, meta_index, mode, ref_count_ptr))
    }

    /// Set reference count for a buffer
    pub fn set_ref_count(&self, meta_index: u32, count: i32) -> Result<()> {
        let meta = self.meta_region.get(meta_index)?;
        meta.ref_count.store(count, Ordering::SeqCst);
        Ok(())
    }

    /// Add reference to a buffer
    pub fn add_ref(&self, meta_index: u32) -> Result<i32> {
        let meta = self.meta_region.get(meta_index)?;
        Ok(meta.ref_count.fetch_add(1, Ordering::SeqCst) + 1)
    }

    /// Release a buffer (decrement ref count)
    pub fn release(&self, meta_index: u32) -> Result<i32> {
        let meta = self.meta_region.get(meta_index)?;
        Ok(meta.ref_count.fetch_sub(1, Ordering::SeqCst) - 1)
    }

    /// Get current reference count
    pub fn ref_count(&self, meta_index: u32) -> Result<i32> {
        let meta = self.meta_region.get(meta_index)?;
        Ok(meta.ref_count.load(Ordering::SeqCst))
    }
}
```

**Step 2: 更新 lib.rs**

```rust
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
```

**Step 3: 验证编译**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/pool.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add BufferPool implementation"
```

---

## Task 4: BufferPool 单元测试

**Files:**
- Modify: `crates/xmem-core/src/pool.rs`

**Step 1: 添加测试**

在 `pool.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn unique_name() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/xmem_pool_test_{}", ts)
    }

    #[test]
    fn test_create_pool() {
        let name = unique_name();
        let pool = BufferPool::create(&name).unwrap();
        assert_eq!(pool.name(), name);
        assert_eq!(pool.capacity(), DEFAULT_CAPACITY);
    }

    #[test]
    fn test_acquire_cpu_buffer() {
        let name = unique_name();
        let pool = BufferPool::create(&name).unwrap();

        let mut buf = pool.acquire_cpu(1024).unwrap();
        assert_eq!(buf.meta_index(), 0);

        // Write data
        let data = b"hello";
        buf.as_cpu_slice_mut().unwrap()[..data.len()].copy_from_slice(data);

        // Read back
        assert_eq!(&buf.as_cpu_slice().unwrap()[..data.len()], data);
    }

    #[test]
    fn test_get_buffer() {
        let name = unique_name();
        let pool = BufferPool::create(&name).unwrap();

        // Acquire and write
        let meta_index;
        {
            let mut buf = pool.acquire_cpu(1024).unwrap();
            meta_index = buf.meta_index();
            buf.as_cpu_slice_mut().unwrap()[..5].copy_from_slice(b"hello");
            pool.set_ref_count(meta_index, 2).unwrap(); // Keep alive
        }

        // Get and read
        {
            let buf = pool.get(meta_index).unwrap();
            assert_eq!(&buf.as_cpu_slice().unwrap()[..5], b"hello");
        }
    }

    #[test]
    fn test_ref_count() {
        let name = unique_name();
        let pool = BufferPool::create(&name).unwrap();

        let buf = pool.acquire_cpu(1024).unwrap();
        let meta_index = buf.meta_index();

        assert_eq!(pool.ref_count(meta_index).unwrap(), 1);

        pool.add_ref(meta_index).unwrap();
        assert_eq!(pool.ref_count(meta_index).unwrap(), 2);

        pool.release(meta_index).unwrap();
        assert_eq!(pool.ref_count(meta_index).unwrap(), 1);
    }

    #[test]
    fn test_read_only_guard() {
        let name = unique_name();
        let pool = BufferPool::create(&name).unwrap();

        let meta_index;
        {
            let buf = pool.acquire_cpu(1024).unwrap();
            meta_index = buf.meta_index();
            pool.set_ref_count(meta_index, 2).unwrap();
        }

        // Get read-only
        let mut buf = pool.get(meta_index).unwrap();
        assert!(buf.as_cpu_slice().is_ok());
        assert!(buf.as_cpu_slice_mut().is_err()); // Should fail
    }

    #[test]
    fn test_forget() {
        let name = unique_name();
        let pool = BufferPool::create(&name).unwrap();

        let meta_index;
        {
            let buf = pool.acquire_cpu(1024).unwrap();
            meta_index = buf.meta_index();
            buf.forget(); // Don't decrement ref count
        }

        // Ref count should still be 1
        assert_eq!(pool.ref_count(meta_index).unwrap(), 1);
    }
}
```

**Step 2: 运行测试**

Run: `cargo test pool`
Expected: PASS (6 tests)

**Step 3: Commit**

```bash
git add crates/xmem-core/src/pool.rs
git commit -m "test(core): add BufferPool tests"
```

---

## Task 5: 预分配接口

**Files:**
- Modify: `crates/xmem-core/src/pool.rs`

**Step 1: 添加 preallocate_cpu 方法**

在 `BufferPool` impl 块中添加：

```rust
    /// Preallocate CPU buffers
    pub fn preallocate_cpu(&self, size: usize, count: usize) -> Result<Vec<u32>> {
        let mut indices = Vec::with_capacity(count);

        for _ in 0..count {
            let buf = self.acquire_cpu(size)?;
            let meta_index = buf.meta_index();
            buf.forget(); // Keep buffer alive
            indices.push(meta_index);
        }

        Ok(indices)
    }
```

**Step 2: 添加测试**

在测试模块中添加：

```rust
    #[test]
    fn test_preallocate_cpu() {
        let name = unique_name();
        let pool = BufferPool::create(&name).unwrap();

        let indices = pool.preallocate_cpu(1024, 5).unwrap();
        assert_eq!(indices.len(), 5);

        // All should have ref_count = 1
        for &idx in &indices {
            assert_eq!(pool.ref_count(idx).unwrap(), 1);
        }
    }
```

**Step 3: 运行测试**

Run: `cargo test preallocate`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/pool.rs
git commit -m "feat(core): add preallocate_cpu method"
```

---

## Phase 3 完成检查

Run: `cargo test`
Expected: 所有测试通过

**产出文件：**
```
crates/xmem-core/src/
├── lib.rs
├── error.rs
├── dtype.rs
├── storage.rs
├── meta.rs
├── shm.rs
├── meta_region.rs
├── buffer.rs       # NEW
├── guard.rs        # NEW
└── pool.rs         # NEW
```
