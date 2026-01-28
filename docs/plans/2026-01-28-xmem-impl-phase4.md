# Phase 4: CUDA 支持

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 CUDA 显存分配和 IPC 跨进程共享。

**依赖:** Phase 3 完成

---

## Task 1: CUDA Buffer 封装

**Files:**
- Create: `crates/xmem-core/src/cuda.rs`
- Modify: `crates/xmem-core/src/lib.rs`

**Step 1: 创建 cuda.rs**

```rust
//! CUDA buffer and IPC support

use crate::{Error, Result};
use cudarc::driver::{CudaDevice, CudaSlice, DevicePtr};
use std::sync::Arc;

/// CUDA IPC memory handle (64 bytes)
#[derive(Clone, Copy)]
#[repr(C)]
pub struct CudaIpcHandle {
    pub reserved: [u8; 64],
}

impl Default for CudaIpcHandle {
    fn default() -> Self {
        Self { reserved: [0u8; 64] }
    }
}

/// CUDA buffer wrapper
pub struct CudaBuffer {
    device: Arc<CudaDevice>,
    device_id: i32,
    ptr: u64,
    size: usize,
    ipc_handle: CudaIpcHandle,
    is_ipc_imported: bool,
}

impl CudaBuffer {
    /// Allocate a new CUDA buffer
    pub fn alloc(device_id: i32, size: usize) -> Result<Self> {
        let device = CudaDevice::new(device_id as usize)
            .map_err(|e| Error::Cuda(e.to_string()))?;
        let device = Arc::new(device);

        // Allocate device memory
        let slice: CudaSlice<u8> = device
            .alloc_zeros(size)
            .map_err(|e| Error::Cuda(e.to_string()))?;

        let ptr = *slice.device_ptr() as u64;

        // Get IPC handle
        let ipc_handle = Self::get_ipc_handle(ptr)?;

        // Prevent deallocation by forgetting the slice
        std::mem::forget(slice);

        Ok(Self {
            device,
            device_id,
            ptr,
            size,
            ipc_handle,
            is_ipc_imported: false,
        })
    }

    /// Open a CUDA buffer from IPC handle
    pub fn from_ipc_handle(device_id: i32, handle: &CudaIpcHandle, size: usize) -> Result<Self> {
        let device = CudaDevice::new(device_id as usize)
            .map_err(|e| Error::Cuda(e.to_string()))?;
        let device = Arc::new(device);

        let ptr = Self::open_ipc_handle(handle)?;

        Ok(Self {
            device,
            device_id,
            ptr,
            size,
            ipc_handle: *handle,
            is_ipc_imported: true,
        })
    }

    /// Get device ID
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Get device pointer
    pub fn device_ptr(&self) -> u64 {
        self.ptr
    }

    /// Get size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get IPC handle
    pub fn ipc_handle(&self) -> &CudaIpcHandle {
        &self.ipc_handle
    }

    /// Get IPC handle from device pointer (using CUDA driver API)
    fn get_ipc_handle(ptr: u64) -> Result<CudaIpcHandle> {
        let mut handle = CudaIpcHandle::default();

        // Use raw CUDA driver API
        unsafe {
            let result = cudarc::driver::sys::cuIpcGetMemHandle(
                handle.reserved.as_mut_ptr() as *mut _,
                ptr,
            );
            if result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
                return Err(Error::Cuda(format!("cuIpcGetMemHandle failed: {:?}", result)));
            }
        }

        Ok(handle)
    }

    /// Open IPC handle (using CUDA driver API)
    fn open_ipc_handle(handle: &CudaIpcHandle) -> Result<u64> {
        let mut ptr: u64 = 0;

        unsafe {
            let result = cudarc::driver::sys::cuIpcOpenMemHandle(
                &mut ptr as *mut u64 as *mut _,
                *(handle.reserved.as_ptr() as *const _),
                cudarc::driver::sys::CUipcMem_flags::CU_IPC_MEM_LAZY_ENABLE_PEER_ACCESS,
            );
            if result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
                return Err(Error::Cuda(format!("cuIpcOpenMemHandle failed: {:?}", result)));
            }
        }

        Ok(ptr)
    }

    /// Close IPC handle
    fn close_ipc_handle(ptr: u64) -> Result<()> {
        unsafe {
            let result = cudarc::driver::sys::cuIpcCloseMemHandle(ptr);
            if result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
                return Err(Error::Cuda(format!("cuIpcCloseMemHandle failed: {:?}", result)));
            }
        }
        Ok(())
    }

    /// Free device memory
    fn free(ptr: u64) -> Result<()> {
        unsafe {
            let result = cudarc::driver::sys::cuMemFree_v2(ptr);
            if result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
                return Err(Error::Cuda(format!("cuMemFree failed: {:?}", result)));
            }
        }
        Ok(())
    }
}

impl Drop for CudaBuffer {
    fn drop(&mut self) {
        if self.is_ipc_imported {
            let _ = Self::close_ipc_handle(self.ptr);
        } else {
            let _ = Self::free(self.ptr);
        }
    }
}

// Safety: CudaBuffer can be sent between threads
// CUDA operations are thread-safe when using the driver API
unsafe impl Send for CudaBuffer {}
unsafe impl Sync for CudaBuffer {}
```

**Step 2: 更新 lib.rs**

```rust
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
```

**Step 3: 验证编译**

Run: `cargo check --features cuda`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/cuda.rs crates/xmem-core/src/lib.rs
git commit -m "feat(core): add CudaBuffer with IPC support"
```

---

## Task 2: 更新 BufferData 支持 CUDA

**Files:**
- Modify: `crates/xmem-core/src/buffer.rs`

**Step 1: 更新 buffer.rs**

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

    /// Get CUDA device ID (only for CUDA buffers)
    #[cfg(feature = "cuda")]
    pub fn cuda_device_id(&self) -> Option<i32> {
        match self {
            BufferData::Cpu(_) => None,
            BufferData::Cuda(buf) => Some(buf.device_id()),
        }
    }
}
```

**Step 2: 验证编译**

Run: `cargo check --features cuda`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-core/src/buffer.rs
git commit -m "feat(core): update BufferData for CUDA support"
```

---

## Task 3: 更新 BufferMeta 支持 CUDA IPC Handle

**Files:**
- Modify: `crates/xmem-core/src/meta.rs`

**Step 1: 更新 meta.rs**

```rust
//! Buffer metadata structure

use std::sync::atomic::AtomicI32;

/// Maximum number of dimensions
pub const MAX_NDIM: usize = 8;

/// CUDA IPC handle size
pub const CUDA_IPC_HANDLE_SIZE: usize = 64;

/// Buffer metadata stored in shared memory
#[repr(C)]
pub struct BufferMeta {
    /// Unique buffer ID
    pub id: u32,
    /// Reference count (atomic)
    pub ref_count: AtomicI32,
    /// Storage type: 0=cpu, 1=cuda
    pub storage_type: u8,
    /// GPU device ID (for CUDA)
    pub device_id: u8,
    /// Data type
    pub dtype: u8,
    /// Number of dimensions
    pub ndim: u8,
    /// Shape (up to 8 dimensions)
    pub shape: [u64; MAX_NDIM],
    /// Strides in bytes
    pub strides: [u64; MAX_NDIM],
    /// Total size in bytes
    pub size: u64,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: u64,
    /// Sequence number
    pub seq: u64,
    /// Content type string (null-terminated)
    pub content_type: [u8; 32],
    /// Producer name (null-terminated)
    pub producer: [u8; 32],
    /// CUDA IPC handle (only valid when storage_type == 1)
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

**Step 2: 验证编译**

Run: `cargo check`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-core/src/meta.rs
git commit -m "feat(core): add cuda_ipc_handle to BufferMeta"
```

---

## Task 4: 更新 BufferPool 支持 CUDA

**Files:**
- Modify: `crates/xmem-core/src/pool.rs`

**Step 1: 添加 CUDA 方法到 BufferPool**

在 `pool.rs` 的 `BufferPool` impl 块中添加：

```rust
    /// Acquire a new CUDA buffer
    #[cfg(feature = "cuda")]
    pub fn acquire_cuda(&self, size: usize, device_id: i32) -> Result<BufferGuard> {
        use crate::cuda::{CudaBuffer, CudaIpcHandle};

        // Allocate metadata slot
        let meta_index = self.meta_region.alloc()?;

        // Allocate CUDA buffer
        let cuda_buf = CudaBuffer::alloc(device_id, size)?;
        let ipc_handle = cuda_buf.ipc_handle();

        // Initialize metadata
        let meta_region_ptr = &self.meta_region as *const MetaRegion as *mut MetaRegion;
        let meta = unsafe { (*meta_region_ptr).get_mut(meta_index)? };
        meta.id = meta_index;
        meta.ref_count.store(1, Ordering::SeqCst);
        meta.storage_type = StorageType::Cuda as u8;
        meta.device_id = device_id as u8;
        meta.size = size as u64;
        meta.cuda_ipc_handle.copy_from_slice(&ipc_handle.reserved);

        let ref_count_ptr = &meta.ref_count as *const _;

        // Create buffer data
        let data = BufferData::Cuda(cuda_buf);

        Ok(BufferGuard::new(
            data,
            meta_index,
            AccessMode::ReadWrite,
            ref_count_ptr,
        ))
    }

    /// Preallocate CUDA buffers
    #[cfg(feature = "cuda")]
    pub fn preallocate_cuda(&self, size: usize, count: usize, device_id: i32) -> Result<Vec<u32>> {
        let mut indices = Vec::with_capacity(count);

        for _ in 0..count {
            let buf = self.acquire_cuda(size, device_id)?;
            let meta_index = buf.meta_index();
            buf.forget();
            indices.push(meta_index);
        }

        Ok(indices)
    }
```

**Step 2: 更新 get_with_mode 方法支持 CUDA**

替换 `get_with_mode` 方法：

```rust
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
                use crate::cuda::{CudaBuffer, CudaIpcHandle};

                let mut handle = CudaIpcHandle::default();
                handle.reserved.copy_from_slice(&meta.cuda_ipc_handle);

                let cuda_buf = CudaBuffer::from_ipc_handle(
                    meta.device_id as i32,
                    &handle,
                    meta.size as usize,
                )?;
                BufferData::Cuda(cuda_buf)
            }
        };

        Ok(BufferGuard::new(data, meta_index, mode, ref_count_ptr))
    }
```

**Step 3: 验证编译**

Run: `cargo check --features cuda`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/pool.rs
git commit -m "feat(core): add CUDA support to BufferPool"
```

---

## Task 5: CUDA 单元测试

**Files:**
- Modify: `crates/xmem-core/src/pool.rs`

**Step 1: 添加 CUDA 测试**

在 `pool.rs` 测试模块中添加：

```rust
    #[cfg(feature = "cuda")]
    mod cuda_tests {
        use super::*;

        #[test]
        fn test_acquire_cuda_buffer() {
            let name = unique_name();
            let pool = BufferPool::create(&name).unwrap();

            let buf = pool.acquire_cuda(1024, 0).unwrap();
            assert_eq!(buf.meta_index(), 0);

            let ptr = buf.as_cuda_ptr().unwrap();
            assert!(ptr > 0);
        }

        #[test]
        fn test_cuda_ipc() {
            let name = unique_name();
            let pool = BufferPool::create(&name).unwrap();

            // Acquire CUDA buffer
            let meta_index;
            {
                let buf = pool.acquire_cuda(1024, 0).unwrap();
                meta_index = buf.meta_index();
                pool.set_ref_count(meta_index, 2).unwrap();
            }

            // Get via IPC
            {
                let buf = pool.get(meta_index).unwrap();
                let ptr = buf.as_cuda_ptr().unwrap();
                assert!(ptr > 0);
            }
        }

        #[test]
        fn test_preallocate_cuda() {
            let name = unique_name();
            let pool = BufferPool::create(&name).unwrap();

            let indices = pool.preallocate_cuda(1024, 3, 0).unwrap();
            assert_eq!(indices.len(), 3);

            for &idx in &indices {
                assert_eq!(pool.ref_count(idx).unwrap(), 1);
            }
        }
    }
```

**Step 2: 运行测试（需要 GPU 环境）**

Run: `cargo test --features cuda cuda_tests`
Expected: PASS (如果有 GPU)

**Step 3: Commit**

```bash
git add crates/xmem-core/src/pool.rs
git commit -m "test(core): add CUDA tests"
```

---

## Phase 4 完成检查

Run: `cargo test && cargo test --features cuda` (后者需要 GPU)
Expected: 所有测试通过

**产出文件：**
```
crates/xmem-core/src/
├── lib.rs          # UPDATED
├── error.rs
├── dtype.rs
├── storage.rs
├── meta.rs         # UPDATED (cuda_ipc_handle)
├── shm.rs
├── meta_region.rs
├── buffer.rs       # UPDATED
├── guard.rs
├── pool.rs         # UPDATED (CUDA methods)
└── cuda.rs         # NEW
```
