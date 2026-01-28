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

#[pymodule]
fn xmem(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<BufferPool>()?;
    m.add_class::<BufferGuard>()?;
    Ok(())
}
