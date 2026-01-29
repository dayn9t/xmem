//! Buffer pool management
//!
//! 提供 [`BufferPool`] 类型用于管理跨进程共享内存缓冲池。
//!
//! # 示例
//!
//! ```
//! use xmem_core::BufferPool;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 创建新池
//! let pool = BufferPool::create("/my_pool_doc")?;
//!
//! // 分配 CPU buffer
//! let mut buf = pool.acquire_cpu(16)?;
//! buf.as_cpu_slice_mut()?.copy_from_slice(b"hello world!!!!!"); // 16 bytes
//!
//! // 获取已存在的 buffer
//! let buf = pool.get(0)?;
//! let data = buf.as_cpu_slice()?;
//! assert_eq!(data, b"hello world!!!!!");
//! # Ok(())
//! # }
//! ```

use crate::buffer::BufferData;
use crate::guard::BufferGuard;
use crate::meta_region::MetaRegion;
use crate::shm::SharedMemory;
use crate::storage::{AccessMode, StorageType};
use crate::{Error, Result};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Default metadata region capacity
const DEFAULT_CAPACITY: usize = 1024;

/// 跨进程共享内存缓冲池
///
/// 管理共享内存缓冲区的分配、访问和生命周期。
///
/// # 示例
///
/// ```
/// use xmem_core::BufferPool;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // 创建新池
/// let pool = BufferPool::create("/my_pool_example")?;
///
/// // 分配 buffer
/// let mut buf = pool.acquire_cpu(16)?;
/// let idx = buf.meta_index();
///
/// // 传递 idx 给其他进程，然后打开
/// let buf = pool.get(idx)?;
/// # Ok(())
/// # }
/// ```
pub struct BufferPool {
    /// Pool name
    name: String,
    /// Metadata region
    meta_region: MetaRegion,
}

impl BufferPool {
    /// 创建一个新的缓冲池
    ///
    /// # 参数
    ///
    /// - `name`: 池名称，用于标识共享内存区域
    ///
    /// # 示例
    ///
    /// ```
    /// use xmem_core::BufferPool;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let pool = BufferPool::create("/my_pool")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(name: &str) -> Result<Self> {
        Self::create_with_capacity(name, DEFAULT_CAPACITY)
    }

    /// 创建指定容量的缓冲池
    ///
    /// # 参数
    ///
    /// - `name`: 池名称
    /// - `capacity`: 最大 buffer 数量
    pub fn create_with_capacity(name: &str, capacity: usize) -> Result<Self> {
        let meta_name = format!("{}_meta", name);
        let meta_region = MetaRegion::create(&meta_name, capacity)?;

        Ok(Self {
            name: name.to_string(),
            meta_region,
        })
    }

    /// Open an existing buffer pool
    pub fn open(name: &str) -> Result<Self> {
        let meta_name = format!("{}_meta", name);
        let meta_region = MetaRegion::open(&meta_name)?;

        Ok(Self {
            name: name.to_string(),
            meta_region,
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

    /// Acquire a buffer, blocking if pool is full
    pub fn acquire_cpu_blocking(&self, size: usize, timeout: Duration) -> Result<BufferGuard> {
        let start = std::time::Instant::now();

        loop {
            match self.acquire_cpu(size) {
                Ok(buf) => return Ok(buf),
                Err(Error::SharedMemory(msg)) if msg.contains("full") => {
                    if start.elapsed() >= timeout {
                        return Err(Error::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Acquire a new CPU buffer
    pub fn acquire_cpu(&self, size: usize) -> Result<BufferGuard> {
        // Allocate metadata slot
        let meta_index = self.meta_region.alloc()?;

        // Create shared memory for buffer data
        let shm_name = self.buffer_shm_name(meta_index);
        let shm = SharedMemory::create(&shm_name, size)?;

        // Initialize metadata
        let meta = self.meta_region.get(meta_index)?;
        meta.id.store(meta_index, Ordering::SeqCst);
        meta.ref_count.store(1, Ordering::SeqCst);
        meta.storage_type.store(StorageType::Cpu as u8, Ordering::SeqCst);
        meta.device_id.store(0, Ordering::SeqCst);
        meta.size.store(size as u64, Ordering::SeqCst);

        let ref_count_ptr = &meta.ref_count as *const _;

        // Create buffer data
        let data = BufferData::Cpu(shm);

        Ok(BufferGuard::new(
            data,
            meta_index,
            AccessMode::ReadWrite,
            ref_count_ptr,
        ).with_pool(self.name.clone()))
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
        let storage_type_val = meta.storage_type.load(Ordering::SeqCst);
        let storage_type = StorageType::from_u8(storage_type_val)
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
                    meta.device_id.load(Ordering::SeqCst) as i32,
                    &handle,
                    meta.size.load(Ordering::SeqCst) as usize,
                )?;
                BufferData::Cuda(cuda_buf)
            }
        };

        Ok(BufferGuard::new(data, meta_index, mode, ref_count_ptr)
            .with_pool(self.name.clone()))
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

    /// Release a buffer back to the pool (called when ref_count reaches 0)
    pub fn release_buffer(&self, meta_index: u32) -> Result<()> {
        // Note: SharedMemory for buffer data is NOT unlinked
        // It will be reused when this meta_index is allocated again
        self.meta_region.free(meta_index)
    }

    /// Check if a buffer should be released (ref_count == 0)
    pub fn try_release(&self, meta_index: u32) -> Result<bool> {
        let meta = self.meta_region.get(meta_index)?;
        let ref_count = meta.ref_count.load(Ordering::SeqCst);

        if ref_count <= 0 {
            self.release_buffer(meta_index)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

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

    /// Acquire a new CUDA buffer
    #[cfg(feature = "cuda")]
    pub fn acquire_cuda(&self, size: usize, device_id: i32) -> Result<BufferGuard> {
        use crate::cuda::CudaBuffer;

        // Allocate metadata slot
        let meta_index = self.meta_region.alloc()?;

        // Allocate CUDA buffer
        let cuda_buf = CudaBuffer::alloc(device_id, size)?;
        let ipc_handle = cuda_buf.ipc_handle();

        // Initialize metadata
        let meta = self.meta_region.get(meta_index)?;
        meta.id.store(meta_index, Ordering::SeqCst);
        meta.ref_count.store(1, Ordering::SeqCst);
        meta.storage_type.store(StorageType::Cuda as u8, Ordering::SeqCst);
        meta.device_id.store(device_id as u8, Ordering::SeqCst);
        meta.size.store(size as u64, Ordering::SeqCst);

        // Copy IPC handle to metadata
        unsafe {
            std::ptr::copy_nonoverlapping(
                ipc_handle.reserved.as_ptr(),
                meta.cuda_ipc_handle.as_ptr() as *mut u8,
                64,
            );
        }

        let ref_count_ptr = &meta.ref_count as *const _;

        // Create buffer data
        let data = BufferData::Cuda(cuda_buf);

        Ok(BufferGuard::new(
            data,
            meta_index,
            AccessMode::ReadWrite,
            ref_count_ptr,
        ).with_pool(self.name.clone()))
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
}

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
        let mut buf = pool.acquire_cpu(1024).unwrap();
        let meta_index = buf.meta_index();
        buf.as_cpu_slice_mut().unwrap()[..5].copy_from_slice(b"hello");
        pool.set_ref_count(meta_index, 2).unwrap(); // Keep alive after drop

        // Get and read (original buf still alive, so shm exists)
        let buf2 = pool.get(meta_index).unwrap();
        assert_eq!(&buf2.as_cpu_slice().unwrap()[..5], b"hello");
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

        let buf = pool.acquire_cpu(1024).unwrap();
        let meta_index = buf.meta_index();
        pool.set_ref_count(meta_index, 2).unwrap();

        // Get read-only (original buf still alive)
        let mut buf2 = pool.get(meta_index).unwrap();
        assert!(buf2.as_cpu_slice().is_ok());
        assert!(buf2.as_cpu_slice_mut().is_err()); // Should fail
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

    #[test]
    fn test_buffer_recycle() {
        let name = unique_name();
        let pool = BufferPool::create_with_capacity(&name, 3).unwrap();

        // Allocate all 3 slots
        let buf0 = pool.acquire_cpu(1024).unwrap();
        let buf1 = pool.acquire_cpu(1024).unwrap();
        let buf2 = pool.acquire_cpu(1024).unwrap();

        assert_eq!(buf0.meta_index(), 0);
        assert_eq!(buf1.meta_index(), 1);
        assert_eq!(buf2.meta_index(), 2);

        // Pool should be full
        assert!(pool.acquire_cpu(1024).is_err());

        // Drop buf1 - should recycle
        drop(buf1);

        // Now we can allocate again, should get recycled index
        let buf3 = pool.acquire_cpu(1024).unwrap();
        assert_eq!(buf3.meta_index(), 1);  // Recycled!
    }

    #[test]
    fn test_buffer_recycle_with_multiple_refs() {
        let name = unique_name();
        let pool = BufferPool::create_with_capacity(&name, 2).unwrap();

        // Allocate
        let buf0 = pool.acquire_cpu(1024).unwrap();
        let idx = buf0.meta_index();

        // Add another reference
        pool.add_ref(idx).unwrap();
        assert_eq!(pool.ref_count(idx).unwrap(), 2);

        // Drop first ref
        drop(buf0);
        assert_eq!(pool.ref_count(idx).unwrap(), 1);

        // Buffer should NOT be recycled yet
        let buf1 = pool.acquire_cpu(1024).unwrap();
        assert_eq!(buf1.meta_index(), 1);  // New slot, not recycled

        // Release second ref manually
        pool.release(idx).unwrap();

        // Now try_release should recycle
        assert!(pool.try_release(idx).unwrap());
    }

    #[test]
    fn test_acquire_blocking_timeout() {
        let name = unique_name();
        let pool = BufferPool::create_with_capacity(&name, 1).unwrap();

        // Fill the pool
        let _buf = pool.acquire_cpu(1024).unwrap();

        // Try to acquire with timeout - should fail
        let result = pool.acquire_cpu_blocking(1024, Duration::from_millis(50));
        assert!(matches!(result, Err(Error::Timeout)));
    }

    #[test]
    fn test_acquire_blocking_success() {
        use std::thread;

        let name = unique_name();
        let pool = BufferPool::create_with_capacity(&name, 1).unwrap();

        // Fill the pool
        let buf = pool.acquire_cpu(1024).unwrap();

        // Spawn thread to release after delay
        let pool_name = name.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            drop(buf);
        });

        // This should succeed after the buffer is released
        let pool2 = BufferPool::open(&pool_name).unwrap();
        let result = pool2.acquire_cpu_blocking(1024, Duration::from_millis(100));
        assert!(result.is_ok());
    }
}

#[cfg(all(test, feature = "cuda"))]
mod cuda_tests {
    use super::*;

    fn unique_name() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/xmem_cuda_test_{}", ts)
    }

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
        let buf = pool.acquire_cuda(1024, 0).unwrap();
        let meta_index = buf.meta_index();
        pool.set_ref_count(meta_index, 2).unwrap();

        // Get via IPC (original buf still alive)
        let buf2 = pool.get(meta_index).unwrap();
        let ptr = buf2.as_cuda_ptr().unwrap();
        assert!(ptr > 0);
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
