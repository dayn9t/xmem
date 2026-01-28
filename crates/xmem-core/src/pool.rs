//! Buffer pool management

use crate::buffer::BufferData;
use crate::guard::BufferGuard;
use crate::meta_region::MetaRegion;
use crate::shm::SharedMemory;
use crate::storage::{AccessMode, StorageType};
use crate::{Error, Result};
use std::sync::atomic::Ordering;

/// Default metadata region capacity
const DEFAULT_CAPACITY: usize = 1024;

/// Buffer pool for managing shared memory buffers
pub struct BufferPool {
    /// Pool name
    name: String,
    /// Metadata region
    meta_region: MetaRegion,
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

        // Open buffer data based on storage type (Phase 3: CPU only)
        let storage_type_val = meta.storage_type.load(Ordering::SeqCst);
        let storage_type = StorageType::from_u8(storage_type_val)
            .ok_or_else(|| Error::SharedMemory("invalid storage type".to_string()))?;

        let data = match storage_type {
            StorageType::Cpu => {
                let shm_name = self.buffer_shm_name(meta_index);
                let shm = SharedMemory::open(&shm_name)?;
                BufferData::Cpu(shm)
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
