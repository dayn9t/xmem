//! Python bindings for xmem

use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use std::sync::Arc;
use xmem_core::{BufferPool as CorePool, AccessMode};

/// Convert xmem error to Python exception
fn to_py_err(e: xmem_core::Error) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// Python wrapper for BufferPool
#[pyclass(unsendable)]
struct BufferPool {
    inner: Arc<CorePool>,
}

/// Python wrapper for BufferGuard
///
/// 持有 pool 的 Arc 引用，确保 pool 在 guard 存活期间不会被释放
#[pyclass(unsendable)]
struct BufferGuard {
    pool: Arc<CorePool>,
    meta_index: u32,
    mode: AccessMode,
    /// 是否已调用 forget()
    forgotten: bool,
}

#[pymethods]
impl BufferPool {
    /// Create a new buffer pool
    #[new]
    #[pyo3(signature = (name, capacity=1024))]
    fn new(name: &str, capacity: usize) -> PyResult<Self> {
        let inner = CorePool::create_with_capacity(name, capacity)
            .map_err(to_py_err)?;
        Ok(Self { inner: Arc::new(inner) })
    }

    /// Open an existing buffer pool
    #[staticmethod]
    fn open(name: &str) -> PyResult<Self> {
        let inner = CorePool::open(name).map_err(to_py_err)?;
        Ok(Self { inner: Arc::new(inner) })
    }

    /// Get pool name
    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }

    /// Get capacity
    #[getter]
    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Acquire a CPU buffer
    fn acquire_cpu(&self, size: usize) -> PyResult<BufferGuard> {
        // 先获取 guard 以分配 buffer，然后立即 forget 以转移所有权
        let guard = self.inner.acquire_cpu(size).map_err(to_py_err)?;
        let meta_index = guard.meta_index();
        guard.forget(); // 不减少引用计数，由 PyBufferGuard 管理

        Ok(BufferGuard {
            pool: Arc::clone(&self.inner),
            meta_index,
            mode: AccessMode::ReadWrite,
            forgotten: false,
        })
    }

    /// Acquire a CUDA buffer
    #[cfg(feature = "cuda")]
    fn acquire_cuda(&self, size: usize, device_id: i32) -> PyResult<BufferGuard> {
        let guard = self.inner.acquire_cuda(size, device_id).map_err(to_py_err)?;
        let meta_index = guard.meta_index();
        guard.forget();

        Ok(BufferGuard {
            pool: Arc::clone(&self.inner),
            meta_index,
            mode: AccessMode::ReadWrite,
            forgotten: false,
        })
    }

    /// Preallocate CPU buffers
    fn preallocate_cpu(&self, size: usize, count: usize) -> PyResult<Vec<u32>> {
        self.inner.preallocate_cpu(size, count).map_err(to_py_err)
    }

    /// Preallocate CUDA buffers
    #[cfg(feature = "cuda")]
    fn preallocate_cuda(&self, size: usize, count: usize, device_id: i32) -> PyResult<Vec<u32>> {
        self.inner.preallocate_cuda(size, count, device_id).map_err(to_py_err)
    }

    /// Get a buffer (read-only)
    fn get(&self, meta_index: u32) -> PyResult<BufferGuard> {
        // 验证 buffer 存在并增加引用计数
        self.inner.add_ref(meta_index).map_err(to_py_err)?;

        Ok(BufferGuard {
            pool: Arc::clone(&self.inner),
            meta_index,
            mode: AccessMode::ReadOnly,
            forgotten: false,
        })
    }

    /// Get a buffer (read-write)
    fn get_mut(&self, meta_index: u32) -> PyResult<BufferGuard> {
        self.inner.add_ref(meta_index).map_err(to_py_err)?;

        Ok(BufferGuard {
            pool: Arc::clone(&self.inner),
            meta_index,
            mode: AccessMode::ReadWrite,
            forgotten: false,
        })
    }

    /// Set reference count
    fn set_ref_count(&self, meta_index: u32, count: i32) -> PyResult<()> {
        self.inner.set_ref_count(meta_index, count).map_err(to_py_err)
    }

    /// Add reference
    fn add_ref(&self, meta_index: u32) -> PyResult<i32> {
        self.inner.add_ref(meta_index).map_err(to_py_err)
    }

    /// Release reference
    fn release(&self, meta_index: u32) -> PyResult<i32> {
        self.inner.release(meta_index).map_err(to_py_err)
    }

    /// Get reference count
    fn ref_count(&self, meta_index: u32) -> PyResult<i32> {
        self.inner.ref_count(meta_index).map_err(to_py_err)
    }
}

#[pymodule]
fn xmem(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<BufferPool>()?;
    m.add_class::<BufferGuard>()?;
    Ok(())
}
