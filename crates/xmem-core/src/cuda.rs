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
