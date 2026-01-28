# Phase 5: Python 绑定

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 使用 PyO3 实现 Python 绑定，支持 CPU 和 CUDA buffer。

**依赖:** Phase 4 完成

---

## Task 1: 创建 xmem-python crate

**Files:**
- Create: `crates/xmem-python/Cargo.toml`
- Create: `crates/xmem-python/src/lib.rs`

**Step 1: 创建 Cargo.toml**

```toml
[package]
name = "xmem-python"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Python bindings for xmem shared memory pool"

[lib]
name = "xmem"
crate-type = ["cdylib"]

[features]
default = []
cuda = ["xmem-core/cuda"]

[dependencies]
xmem-core = { path = "../xmem-core" }
pyo3 = { version = "0.20", features = ["extension-module"] }
```

**Step 2: 创建 src/lib.rs 基础结构**

> **生命周期设计说明**：Python 绑定中 `BufferGuard` 不能直接持有 Rust 的 `CoreGuard<'a>`，
> 因为 Python 对象的生命周期由 GC 管理。解决方案：
> 1. `PyBufferPool` 使用 `Arc<CorePool>` 包装
> 2. `PyBufferGuard` 持有 `Arc<CorePool>` 的克隆 + `meta_index`
> 3. 访问时按需创建临时 `CoreGuard`

```rust
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
#[pyclass]
struct BufferPool {
    inner: Arc<CorePool>,
}

/// Python wrapper for BufferGuard
///
/// 持有 pool 的 Arc 引用，确保 pool 在 guard 存活期间不会被释放
#[pyclass]
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
```

**Step 3: 验证编译**

Run: `cargo check -p xmem-python`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-python/
git commit -m "feat(python): init xmem-python crate"
```

---

## Task 2: 实现 BufferPool Python 绑定

**Files:**
- Modify: `crates/xmem-python/src/lib.rs`

**Step 1: 实现 BufferPool 方法**

```rust
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
```

**Step 2: 验证编译**

Run: `cargo check -p xmem-python`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-python/src/lib.rs
git commit -m "feat(python): implement BufferPool bindings"
```

---

## Task 3: 实现 BufferGuard Python 绑定

**Files:**
- Modify: `crates/xmem-python/src/lib.rs`

**Step 1: 实现 BufferGuard 方法**

```rust
#[pymethods]
impl BufferGuard {
    /// Get metadata index
    #[getter]
    fn meta_index(&self) -> u32 {
        self.meta_index
    }

    /// Check if buffer is valid (not forgotten)
    #[getter]
    fn is_valid(&self) -> bool {
        !self.forgotten
    }

    /// Get CPU pointer as integer (按需创建临时 guard)
    #[getter]
    fn cpu_ptr(&self) -> PyResult<u64> {
        if self.forgotten {
            return Err(PyRuntimeError::new_err("buffer already forgotten"));
        }
        let guard = self.pool.get(self.meta_index).map_err(to_py_err)?;
        let slice = guard.as_cpu_slice().map_err(to_py_err)?;
        let ptr = slice.as_ptr() as u64;
        guard.forget(); // 不减少引用计数
        Ok(ptr)
    }

    /// Get CPU pointer as integer (mutable)
    #[getter]
    fn cpu_ptr_mut(&self) -> PyResult<u64> {
        if self.forgotten {
            return Err(PyRuntimeError::new_err("buffer already forgotten"));
        }
        if self.mode == AccessMode::ReadOnly {
            return Err(PyRuntimeError::new_err("buffer is read-only"));
        }
        let mut guard = self.pool.get_mut(self.meta_index).map_err(to_py_err)?;
        let slice = guard.as_cpu_slice_mut().map_err(to_py_err)?;
        let ptr = slice.as_mut_ptr() as u64;
        guard.forget();
        Ok(ptr)
    }

    /// Get CUDA device pointer
    #[cfg(feature = "cuda")]
    #[getter]
    fn cuda_ptr(&self) -> PyResult<u64> {
        if self.forgotten {
            return Err(PyRuntimeError::new_err("buffer already forgotten"));
        }
        let guard = self.pool.get(self.meta_index).map_err(to_py_err)?;
        let ptr = guard.as_cuda_ptr().map_err(to_py_err)?;
        guard.forget();
        Ok(ptr)
    }

    /// Get CUDA device pointer (mutable)
    #[cfg(feature = "cuda")]
    #[getter]
    fn cuda_ptr_mut(&self) -> PyResult<u64> {
        if self.forgotten {
            return Err(PyRuntimeError::new_err("buffer already forgotten"));
        }
        if self.mode == AccessMode::ReadOnly {
            return Err(PyRuntimeError::new_err("buffer is read-only"));
        }
        let mut guard = self.pool.get_mut(self.meta_index).map_err(to_py_err)?;
        let ptr = guard.as_cuda_ptr_mut().map_err(to_py_err)?;
        guard.forget();
        Ok(ptr)
    }

    /// Get buffer size
    #[getter]
    fn size(&self) -> PyResult<usize> {
        if self.forgotten {
            return Err(PyRuntimeError::new_err("buffer already forgotten"));
        }
        let guard = self.pool.get(self.meta_index).map_err(to_py_err)?;
        let slice = guard.as_cpu_slice().map_err(to_py_err)?;
        let size = slice.len();
        guard.forget();
        Ok(size)
    }

    /// Forget this guard without releasing
    fn forget(&mut self) {
        self.forgotten = true;
    }

    /// Context manager enter
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager exit - release reference
    fn __exit__(
        &mut self,
        _exc_type: Option<&PyAny>,
        _exc_val: Option<&PyAny>,
        _exc_tb: Option<&PyAny>,
    ) -> bool {
        if !self.forgotten {
            let _ = self.pool.release(self.meta_index);
            self.forgotten = true;
        }
        false
    }
}

impl Drop for BufferGuard {
    fn drop(&mut self) {
        if !self.forgotten {
            let _ = self.pool.release(self.meta_index);
        }
    }
}
```

**Step 2: 验证编译**

Run: `cargo check -p xmem-python`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-python/src/lib.rs
git commit -m "feat(python): implement BufferGuard bindings"
```

---

## Task 4: 添加 pyproject.toml

**Files:**
- Create: `crates/xmem-python/pyproject.toml`

**Step 1: 创建 pyproject.toml**

```toml
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[project]
name = "xmem"
version = "0.1.0"
description = "Cross-process shared memory pool with CPU and CUDA support"
readme = "README.md"
license = { text = "MIT" }
requires-python = ">=3.8"
classifiers = [
    "Development Status :: 3 - Alpha",
    "Intended Audience :: Developers",
    "License :: OSI Approved :: MIT License",
    "Programming Language :: Python :: 3",
    "Programming Language :: Python :: 3.8",
    "Programming Language :: Python :: 3.9",
    "Programming Language :: Python :: 3.10",
    "Programming Language :: Python :: 3.11",
    "Programming Language :: Python :: 3.12",
    "Programming Language :: Rust",
]

[tool.maturin]
features = ["pyo3/extension-module"]
```

**Step 2: Commit**

```bash
git add crates/xmem-python/pyproject.toml
git commit -m "feat(python): add pyproject.toml for maturin"
```

---

## Task 5: 添加 Python 类型存根

**Files:**
- Create: `crates/xmem-python/xmem.pyi`

**Step 1: 创建类型存根**

```python
"""Type stubs for xmem Python bindings."""

from typing import List, Optional

class BufferPool:
    """Cross-process shared memory buffer pool."""

    def __init__(self, name: str, capacity: int = 1024) -> None:
        """Create a new buffer pool."""
        ...

    @staticmethod
    def open(name: str) -> "BufferPool":
        """Open an existing buffer pool."""
        ...

    @property
    def name(self) -> str:
        """Get pool name."""
        ...

    @property
    def capacity(self) -> int:
        """Get pool capacity."""
        ...

    def acquire_cpu(self, size: int) -> "BufferGuard":
        """Acquire a CPU buffer."""
        ...

    def acquire_cuda(self, size: int, device_id: int) -> "BufferGuard":
        """Acquire a CUDA buffer (requires cuda feature)."""
        ...

    def preallocate_cpu(self, size: int, count: int) -> List[int]:
        """Preallocate CPU buffers."""
        ...

    def preallocate_cuda(self, size: int, count: int, device_id: int) -> List[int]:
        """Preallocate CUDA buffers (requires cuda feature)."""
        ...

    def get(self, meta_index: int) -> "BufferGuard":
        """Get a buffer (read-only)."""
        ...

    def get_mut(self, meta_index: int) -> "BufferGuard":
        """Get a buffer (read-write)."""
        ...

    def set_ref_count(self, meta_index: int, count: int) -> None:
        """Set reference count."""
        ...

    def add_ref(self, meta_index: int) -> int:
        """Add reference, returns new count."""
        ...

    def release(self, meta_index: int) -> int:
        """Release reference, returns new count."""
        ...

    def ref_count(self, meta_index: int) -> int:
        """Get current reference count."""
        ...


class BufferGuard:
    """RAII guard for buffer access."""

    @property
    def meta_index(self) -> int:
        """Get metadata index."""
        ...

    @property
    def is_valid(self) -> bool:
        """Check if buffer is valid."""
        ...

    @property
    def cpu_ptr(self) -> int:
        """Get CPU pointer (read-only)."""
        ...

    @property
    def cpu_ptr_mut(self) -> int:
        """Get CPU pointer (read-write)."""
        ...

    @property
    def cuda_ptr(self) -> int:
        """Get CUDA device pointer (read-only, requires cuda feature)."""
        ...

    @property
    def cuda_ptr_mut(self) -> int:
        """Get CUDA device pointer (read-write, requires cuda feature)."""
        ...

    @property
    def size(self) -> int:
        """Get buffer size in bytes."""
        ...

    def forget(self) -> None:
        """Forget this guard without releasing the buffer."""
        ...

    def __enter__(self) -> "BufferGuard":
        """Context manager enter."""
        ...

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        """Context manager exit."""
        ...
```

**Step 2: Commit**

```bash
git add crates/xmem-python/xmem.pyi
git commit -m "feat(python): add type stubs"
```

---

## Task 6: Python 测试

**Files:**
- Create: `crates/xmem-python/tests/test_xmem.py`

**Step 1: 创建测试文件**

```python
"""Tests for xmem Python bindings."""

import pytest
import time


def unique_name():
    """Generate unique pool name."""
    return f"/xmem_pytest_{int(time.time() * 1e9)}"


class TestBufferPool:
    """Tests for BufferPool."""

    def test_create_pool(self):
        """Test creating a new pool."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)
        assert pool.name == name
        assert pool.capacity == 1024

    def test_create_with_capacity(self):
        """Test creating pool with custom capacity."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name, capacity=100)
        assert pool.capacity == 100

    def test_acquire_cpu(self):
        """Test acquiring CPU buffer."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        assert buf.meta_index == 0
        assert buf.is_valid
        assert buf.size == 1024

    def test_ref_count(self):
        """Test reference counting."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        meta_index = buf.meta_index

        assert pool.ref_count(meta_index) == 1

        pool.add_ref(meta_index)
        assert pool.ref_count(meta_index) == 2

        pool.release(meta_index)
        assert pool.ref_count(meta_index) == 1

    def test_forget(self):
        """Test forget without release."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        meta_index = buf.meta_index
        buf.forget()

        # Ref count should still be 1
        assert pool.ref_count(meta_index) == 1

    def test_context_manager(self):
        """Test context manager usage."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        meta_index = None
        with pool.acquire_cpu(1024) as buf:
            meta_index = buf.meta_index
            pool.set_ref_count(meta_index, 2)
            assert buf.is_valid

        # After context, ref count should be decremented
        assert pool.ref_count(meta_index) == 1

    def test_preallocate_cpu(self):
        """Test preallocating CPU buffers."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        indices = pool.preallocate_cpu(1024, 5)
        assert len(indices) == 5

        for idx in indices:
            assert pool.ref_count(idx) == 1


class TestBufferGuard:
    """Tests for BufferGuard."""

    def test_cpu_ptr(self):
        """Test getting CPU pointer."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        ptr = buf.cpu_ptr
        assert ptr > 0

    def test_read_only_guard(self):
        """Test read-only guard."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        meta_index = buf.meta_index
        pool.set_ref_count(meta_index, 2)
        buf.forget()

        # Get read-only
        buf = pool.get(meta_index)
        _ = buf.cpu_ptr  # Should work

        with pytest.raises(RuntimeError):
            _ = buf.cpu_ptr_mut  # Should fail
```

**Step 2: Commit**

```bash
git add crates/xmem-python/tests/
git commit -m "test(python): add Python tests"
```

---

## Task 7: 构建和测试

**Step 1: 安装 maturin**

Run: `pip install maturin`

**Step 2: 构建 Python 包（开发模式）**

Run: `cd crates/xmem-python && maturin develop`
Expected: 构建成功

**Step 3: 运行 Python 测试**

Run: `cd crates/xmem-python && pytest tests/ -v`
Expected: 所有测试通过

**Step 4: Commit**

```bash
git add -A
git commit -m "feat(python): complete Python bindings"
```

---

## Phase 5 完成检查

Run:
```bash
cargo test
cd crates/xmem-python && maturin develop && pytest tests/ -v
```
Expected: 所有测试通过

**产出文件：**
```
crates/xmem-python/
├── Cargo.toml
├── pyproject.toml
├── xmem.pyi
├── src/
│   └── lib.rs
└── tests/
    └── test_xmem.py
```
