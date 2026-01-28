# Phase 2: CPU 共享内存实现

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 POSIX 共享内存封装，支持创建、打开、读写共享内存区域。

**依赖:** Phase 1 完成

---

## Task 1: 共享内存封装 - 基础结构

**Files:**
- Create: `crates/xmem-core/src/shm.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 shm.rs 基础结构**

```rust
//! POSIX shared memory wrapper

use crate::{Error, Result};
use shared_memory::{Shmem, ShmemConf};

/// Shared memory region wrapper
pub struct SharedMemory {
    inner: Shmem,
    name: String,
    size: usize,
    owner: bool,
}

impl SharedMemory {
    /// Create a new shared memory region
    pub fn create(name: &str, size: usize) -> Result<Self> {
        let shmem = ShmemConf::new()
            .size(size)
            .os_id(name)
            .create()
            .map_err(|e| Error::SharedMemory(e.to_string()))?;

        Ok(Self {
            inner: shmem,
            name: name.to_string(),
            size,
            owner: true,
        })
    }

    /// Open an existing shared memory region
    pub fn open(name: &str) -> Result<Self> {
        let shmem = ShmemConf::new()
            .os_id(name)
            .open()
            .map_err(|e| Error::SharedMemory(e.to_string()))?;

        let size = shmem.len();

        Ok(Self {
            inner: shmem,
            name: name.to_string(),
            size,
            owner: false,
        })
    }

    /// Get the name of the shared memory region
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the size of the shared memory region
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get a raw pointer to the shared memory
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    /// Get a mutable raw pointer to the shared memory
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_ptr() as *mut u8
    }

    /// Get a slice view of the shared memory
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.as_ptr(), self.size) }
    }

    /// Get a mutable slice view of the shared memory
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr(), self.size) }
    }
}
```

**Step 2: 更新 lib.rs**

```rust
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
```

**Step 3: 验证编译**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/shm.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add SharedMemory wrapper"
```

---

## Task 2: 共享内存单元测试

**Files:**
- Modify: `crates/xmem-core/src/shm.rs`

**Step 1: 添加测试模块**

在 `shm.rs` 末尾添加：

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
        format!("/xmem_test_{}", ts)
    }

    #[test]
    fn test_create_and_write() {
        let name = unique_name();
        let mut shm = SharedMemory::create(&name, 1024).unwrap();

        assert_eq!(shm.size(), 1024);
        assert_eq!(shm.name(), name);

        // Write data
        let data = b"hello xmem";
        shm.as_mut_slice()[..data.len()].copy_from_slice(data);

        // Read back
        assert_eq!(&shm.as_slice()[..data.len()], data);
    }

    #[test]
    fn test_open_existing() {
        let name = unique_name();
        let data = b"shared data";

        // Create and write
        {
            let mut shm = SharedMemory::create(&name, 1024).unwrap();
            shm.as_mut_slice()[..data.len()].copy_from_slice(data);
        }

        // Open and read
        {
            let shm = SharedMemory::open(&name).unwrap();
            assert_eq!(&shm.as_slice()[..data.len()], data);
        }
    }

    #[test]
    fn test_open_nonexistent() {
        let result = SharedMemory::open("/xmem_nonexistent_12345");
        assert!(result.is_err());
    }
}
```

**Step 2: 运行测试**

Run: `cargo test shm`
Expected: PASS (3 tests)

**Step 3: Commit**

```bash
git add crates/xmem-core/src/shm.rs
git commit -m "test(core): add SharedMemory tests"
```

---

## Task 3: 元数据区域管理

**Files:**
- Create: `crates/xmem-core/src/meta_region.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 meta_region.rs**

```rust
//! Shared metadata region management

use crate::meta::BufferMeta;
use crate::shm::SharedMemory;
use crate::{Error, Result};
use std::sync::atomic::{AtomicU32, Ordering};

/// Header for metadata region
#[repr(C)]
struct MetaRegionHeader {
    /// Magic number for validation
    magic: u32,
    /// Version number
    version: u32,
    /// Maximum number of buffers
    capacity: u32,
    /// Next buffer ID to allocate
    next_id: AtomicU32,
}

const MAGIC: u32 = 0x584D454D; // "XMEM"
const VERSION: u32 = 1;

/// Shared metadata region
pub struct MetaRegion {
    shm: SharedMemory,
    capacity: usize,
}

impl MetaRegion {
    /// Calculate required size for given capacity
    fn calc_size(capacity: usize) -> usize {
        std::mem::size_of::<MetaRegionHeader>() + capacity * BufferMeta::SIZE
    }

    /// Create a new metadata region
    pub fn create(name: &str, capacity: usize) -> Result<Self> {
        let size = Self::calc_size(capacity);
        let mut shm = SharedMemory::create(name, size)?;

        // Initialize header
        let header = unsafe { &mut *(shm.as_mut_ptr() as *mut MetaRegionHeader) };
        header.magic = MAGIC;
        header.version = VERSION;
        header.capacity = capacity as u32;
        header.next_id = AtomicU32::new(0);

        Ok(Self { shm, capacity })
    }

    /// Open an existing metadata region
    pub fn open(name: &str) -> Result<Self> {
        let shm = SharedMemory::open(name)?;

        // Validate header
        let header = unsafe { &*(shm.as_ptr() as *const MetaRegionHeader) };
        if header.magic != MAGIC {
            return Err(Error::SharedMemory("invalid magic number".to_string()));
        }
        if header.version != VERSION {
            return Err(Error::SharedMemory(format!(
                "version mismatch: expected {}, got {}",
                VERSION, header.version
            )));
        }

        let capacity = header.capacity as usize;
        Ok(Self { shm, capacity })
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Allocate a new buffer slot, returns meta_index
    pub fn alloc(&self) -> Result<u32> {
        let header = unsafe { &*(self.shm.as_ptr() as *const MetaRegionHeader) };
        let id = header.next_id.fetch_add(1, Ordering::SeqCst);

        if id >= self.capacity as u32 {
            header.next_id.fetch_sub(1, Ordering::SeqCst);
            return Err(Error::SharedMemory("metadata region full".to_string()));
        }

        Ok(id)
    }

    /// Get metadata by index
    pub fn get(&self, index: u32) -> Result<&BufferMeta> {
        if index >= self.capacity as u32 {
            return Err(Error::BufferNotFound(index));
        }

        let offset = std::mem::size_of::<MetaRegionHeader>() + (index as usize) * BufferMeta::SIZE;
        let ptr = unsafe { self.shm.as_ptr().add(offset) as *const BufferMeta };
        Ok(unsafe { &*ptr })
    }

    /// Get mutable metadata by index
    pub fn get_mut(&mut self, index: u32) -> Result<&mut BufferMeta> {
        if index >= self.capacity as u32 {
            return Err(Error::BufferNotFound(index));
        }

        let offset = std::mem::size_of::<MetaRegionHeader>() + (index as usize) * BufferMeta::SIZE;
        let ptr = unsafe { self.shm.as_mut_ptr().add(offset) as *mut BufferMeta };
        Ok(unsafe { &mut *ptr })
    }
}
```

**Step 2: 更新 lib.rs**

```rust
//! xmem - Cross-process shared memory pool

pub mod dtype;
pub mod error;
pub mod meta;
pub mod meta_region;
pub mod shm;
pub mod storage;

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
git add crates/xmem-core/src/meta_region.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add MetaRegion for metadata management"
```

---

## Task 4: 元数据区域测试

**Files:**
- Modify: `crates/xmem-core/src/meta_region.rs`

**Step 1: 添加测试**

在 `meta_region.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn unique_name() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/xmem_meta_test_{}", ts)
    }

    #[test]
    fn test_create_and_alloc() {
        let name = unique_name();
        let region = MetaRegion::create(&name, 10).unwrap();

        assert_eq!(region.capacity(), 10);

        // Allocate slots
        let idx0 = region.alloc().unwrap();
        let idx1 = region.alloc().unwrap();

        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
    }

    #[test]
    fn test_get_meta() {
        let name = unique_name();
        let mut region = MetaRegion::create(&name, 10).unwrap();

        let idx = region.alloc().unwrap();
        let meta = region.get_mut(idx).unwrap();

        meta.id = 42;
        meta.size = 1024;
        meta.ref_count.store(1, Ordering::SeqCst);

        // Read back
        let meta = region.get(idx).unwrap();
        assert_eq!(meta.id, 42);
        assert_eq!(meta.size, 1024);
        assert_eq!(meta.ref_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_open_existing() {
        let name = unique_name();

        // Create and write
        {
            let mut region = MetaRegion::create(&name, 10).unwrap();
            let idx = region.alloc().unwrap();
            let meta = region.get_mut(idx).unwrap();
            meta.id = 123;
        }

        // Open and read
        {
            let region = MetaRegion::open(&name).unwrap();
            let meta = region.get(0).unwrap();
            assert_eq!(meta.id, 123);
        }
    }

    #[test]
    fn test_capacity_limit() {
        let name = unique_name();
        let region = MetaRegion::create(&name, 2).unwrap();

        assert!(region.alloc().is_ok());
        assert!(region.alloc().is_ok());
        assert!(region.alloc().is_err()); // Full
    }
}
```

**Step 2: 运行测试**

Run: `cargo test meta_region`
Expected: PASS (4 tests)

**Step 3: Commit**

```bash
git add crates/xmem-core/src/meta_region.rs
git commit -m "test(core): add MetaRegion tests"
```

---

## Phase 2 完成检查

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
├── shm.rs          # NEW
└── meta_region.rs  # NEW
```
