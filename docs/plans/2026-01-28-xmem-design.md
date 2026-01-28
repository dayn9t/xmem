# xmem 技术设计

## 1. 模块结构

```
xmem/
├── Cargo.toml
├── crates/
│   ├── xmem-core/
│   │   ├── buffer.rs         # Buffer 结构、元数据
│   │   ├── pool.rs           # BufferPool 实现
│   │   ├── shm.rs            # POSIX 共享内存封装
│   │   ├── cuda_ipc.rs       # CUDA IPC handle 封装 (feature = "cuda")
│   │   ├── transfer.rs       # CPU/CUDA 互操作
│   │   └── lib.rs
│   │
│   └── xmem-python/          # Python 绑定 (pyo3)
│       └── lib.rs
│
├── examples/
│   ├── producer.rs
│   └── consumer.rs
│
└── tests/
```

## 2. Cargo Features

```toml
[package]
name = "xmem-core"
version = "0.1.0"
edition = "2021"

[features]
default = ["cpu"]
cpu = []                      # CPU 共享内存（始终可用）
cuda = ["dep:cudarc"]         # CUDA 支持（可选）

[dependencies]
# 共享内存
shared_memory = "0.12"

# CUDA（可选）
cudarc = { version = "0.10", optional = true }

# 通用
serde = { version = "1", features = ["derive"] }
crossbeam = "0.8"
thiserror = "1"
```

**使用方式**：

```bash
# 仅 CPU
cargo build

# CPU + CUDA
cargo build --features cuda
```

## 3. 元数据结构

共享内存区域存放所有 buffer 的元数据，固定大小便于索引：

```rust
#[repr(C)]
pub struct BufferMeta {
    pub id: u32,
    pub ref_count: AtomicI32,         // 原子引用计数
    pub storage_type: u8,             // 0=cpu, 1=cuda
    pub device_id: u8,                // GPU 设备号
    pub dtype: u8,                    // float32, uint8, float16...
    pub ndim: u8,                     // 维度数
    pub shape: [u64; 8],              // 最多 8 维
    pub strides: [u64; 8],            // 步长
    pub size: u64,                    // 总字节数
    pub timestamp: u64,
    pub seq: u64,
    pub content_type: [u8; 32],       // "image/raw", "tensor/float32"
    pub producer: [u8; 32],
    pub reserved: [u8; 64],           // 预留扩展
}
```

## 4. Buffer 存储抽象

```rust
pub enum BufferStorage {
    Cpu {
        shm_name: String,
        ptr: *mut u8,
        size: usize,
    },
    #[cfg(feature = "cuda")]
    Cuda {
        device_id: i32,
        device_ptr: u64,              // CUdeviceptr
        ipc_handle: CudaIpcMemHandle,
        size: usize,
    },
}
```

## 5. RAII 封装

```rust
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

pub struct BufferGuard<'a> {
    pool: &'a BufferPool,
    meta_index: u32,
    storage: BufferStorage,
    mode: AccessMode,
}

impl<'a> BufferGuard<'a> {
    // 只读访问
    pub fn as_cpu_slice(&self) -> Result<&[u8]>;

    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr(&self) -> Result<u64>;

    // 可写访问（只读模式调用会报错）
    pub fn as_cpu_slice_mut(&mut self) -> Result<&mut [u8]>;

    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr_mut(&mut self) -> Result<u64>;

    pub fn meta_index(&self) -> u32;
    pub fn clone_ref(&self) -> BufferGuard<'a>;
}

impl Drop for BufferGuard<'_> {
    fn drop(&mut self) {
        self.pool.release(self.meta_index);
    }
}
```

## 6. BufferPool API

```rust
impl BufferPool {
    /// 连接到共享内存池（或创建）
    pub fn new(name: &str) -> Result<Self>;

    /// 预分配 CPU buffer
    pub fn preallocate_cpu(&self, size: usize, count: usize) -> Result<Vec<u32>>;

    /// 预分配 CUDA buffer
    #[cfg(feature = "cuda")]
    pub fn preallocate_cuda(&self, size: usize, count: usize, device_id: i32) -> Result<Vec<u32>>;

    /// 获取 CPU buffer（生产者，可写）
    pub fn acquire_cpu(&self, size: usize) -> Result<BufferGuard>;

    /// 获取 CUDA buffer（生产者，可写）
    #[cfg(feature = "cuda")]
    pub fn acquire_cuda(&self, size: usize, device_id: i32) -> Result<BufferGuard>;

    /// 访问 buffer（消费者）
    pub fn get(&self, meta_index: u32) -> Result<BufferGuard>;      // 只读
    pub fn get_mut(&self, meta_index: u32) -> Result<BufferGuard>;  // 可写

    /// 设置引用计数（多消费者场景）
    pub fn set_ref_count(&self, meta_index: u32, count: i32);

    /// 手动释放（非 RAII 场景）
    pub fn release(&self, meta_index: u32);
    pub fn add_ref(&self, meta_index: u32);
}
```

## 7. CPU/CUDA 互操作

```rust
impl<'a> BufferGuard<'a> {
    /// CPU → CUDA（复制）
    #[cfg(feature = "cuda")]
    pub fn copy_to_cuda(&self, device_id: i32) -> Result<BufferGuard<'a>>;

    /// CUDA → CPU（复制）
    #[cfg(feature = "cuda")]
    pub fn copy_to_cpu(&self) -> Result<BufferGuard<'a>>;

    /// CUDA → CUDA（同设备或跨设备复制）
    #[cfg(feature = "cuda")]
    pub fn copy_to_device(&self, device_id: i32) -> Result<BufferGuard<'a>>;
}
```

**使用示例**：

```rust
// CPU → CUDA
let cpu_buf = pool.acquire_cpu(size)?;
write_data(cpu_buf.as_cpu_slice_mut()?);
let cuda_buf = cpu_buf.copy_to_cuda(0)?;

// CUDA → CPU
let cuda_buf = pool.get(meta_index)?;
let cpu_buf = cuda_buf.copy_to_cpu()?;

// CUDA 0 → CUDA 1
let buf_gpu0 = pool.acquire_cuda(size, 0)?;
let buf_gpu1 = buf_gpu0.copy_to_device(1)?;
```

## 8. Python 绑定

### Cargo.toml

```toml
[package]
name = "xmem-python"
version = "0.1.0"
edition = "2021"

[lib]
name = "xmem"
crate-type = ["cdylib"]

[features]
default = ["cpu"]
cpu = ["xmem-core/cpu"]
cuda = ["xmem-core/cuda"]

[dependencies]
pyo3 = { version = "0.20", features = ["extension-module"] }
xmem-core = { path = "../xmem-core" }
```

### 绑定实现

```rust
use pyo3::prelude::*;
use xmem_core::{BufferPool as CorePool, BufferGuard as CoreGuard};

#[pyclass]
struct BufferPool {
    inner: CorePool,
}

#[pymethods]
impl BufferPool {
    #[new]
    fn new(name: &str) -> PyResult<Self> {
        Ok(Self { inner: CorePool::new(name)? })
    }

    fn preallocate_cpu(&self, size: usize, count: usize) -> PyResult<Vec<u32>> {
        Ok(self.inner.preallocate_cpu(size, count)?)
    }

    #[cfg(feature = "cuda")]
    fn preallocate_cuda(&self, size: usize, count: usize, device_id: i32) -> PyResult<Vec<u32>> {
        Ok(self.inner.preallocate_cuda(size, count, device_id)?)
    }

    fn acquire_cpu(&self, size: usize) -> PyResult<BufferGuard> {
        Ok(BufferGuard { inner: Some(self.inner.acquire_cpu(size)?) })
    }

    #[cfg(feature = "cuda")]
    fn acquire_cuda(&self, size: usize, device_id: i32) -> PyResult<BufferGuard> {
        Ok(BufferGuard { inner: Some(self.inner.acquire_cuda(size, device_id)?) })
    }

    fn get(&self, meta_index: u32) -> PyResult<BufferGuard> {
        Ok(BufferGuard { inner: Some(self.inner.get(meta_index)?) })
    }

    fn get_mut(&self, meta_index: u32) -> PyResult<BufferGuard> {
        Ok(BufferGuard { inner: Some(self.inner.get_mut(meta_index)?) })
    }

    fn set_ref_count(&self, meta_index: u32, count: i32) {
        self.inner.set_ref_count(meta_index, count);
    }
}

#[pyclass]
struct BufferGuard {
    inner: Option<CoreGuard<'static>>,
}

#[pymethods]
impl BufferGuard {
    #[getter]
    fn meta_index(&self) -> PyResult<u32> {
        Ok(self.inner.as_ref().ok_or("already forgotten")?.meta_index())
    }

    #[getter]
    fn cpu_ptr(&self) -> PyResult<u64> {
        let ptr = self.inner.as_ref().ok_or("already forgotten")?.as_cpu_slice()?;
        Ok(ptr.as_ptr() as u64)
    }

    #[cfg(feature = "cuda")]
    #[getter]
    fn cuda_ptr(&self) -> PyResult<u64> {
        Ok(self.inner.as_ref().ok_or("already forgotten")?.as_cuda_ptr()?)
    }

    #[cfg(feature = "cuda")]
    #[getter]
    fn cuda_ptr_mut(&mut self) -> PyResult<u64> {
        Ok(self.inner.as_mut().ok_or("already forgotten")?.as_cuda_ptr_mut()?)
    }

    #[cfg(feature = "cuda")]
    fn copy_to_cuda(&self, device_id: i32) -> PyResult<BufferGuard> {
        let guard = self.inner.as_ref().ok_or("already forgotten")?;
        Ok(BufferGuard { inner: Some(guard.copy_to_cuda(device_id)?) })
    }

    #[cfg(feature = "cuda")]
    fn copy_to_cpu(&self) -> PyResult<BufferGuard> {
        let guard = self.inner.as_ref().ok_or("already forgotten")?;
        Ok(BufferGuard { inner: Some(guard.copy_to_cpu()?) })
    }

    fn forget(&mut self) {
        if let Some(guard) = self.inner.take() {
            std::mem::forget(guard);
        }
    }
}

#[pymodule]
fn xmem(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<BufferPool>()?;
    m.add_class::<BufferGuard>()?;
    Ok(())
}
```

### 构建

```bash
# 仅 CPU
maturin build

# CPU + CUDA
maturin build --features cuda
```

### Python 使用

```python
from xmem import BufferPool

pool = BufferPool("my_pool")

# 预分配
pool.preallocate_cuda(size=1920*1080*3, count=8, device_id=0)

# 生产者
buf = pool.acquire_cuda(1920*1080*3, device_id=0)
decode_to(buf.cuda_ptr_mut)
pool.set_ref_count(buf.meta_index, 2)
meta_index = buf.meta_index
buf.forget()

# 消费者
buf = pool.get(meta_index)
process(buf.cuda_ptr)

# CPU/CUDA 互操作
cpu_buf = pool.acquire_cpu(size)
cuda_buf = cpu_buf.copy_to_cuda(device_id=0)
```

## 9. 框架集成

### PyTorch

```python
import torch

buf = pool.get(meta_index)
tensor = torch.from_blob(
    buf.cuda_ptr,
    shape=(1080, 1920, 3),
    dtype=torch.uint8,
    device='cuda:0'
)
```

### ONNX Runtime

```python
io_binding = session.io_binding()
io_binding.bind_input(
    name='input',
    device_type='cuda',
    device_id=0,
    element_type=np.float32,
    shape=(1, 3, 224, 224),
    buffer_ptr=buf.cuda_ptr
)
```

## 10. CUDA IPC 流程

```
进程 A (生产者)                    进程 B (消费者)
─────────────────                 ─────────────────
cudaMalloc(&ptr, size)
cudaIpcGetMemHandle(&handle, ptr)
    │
    └──── handle 存入 metadata ────►  cudaIpcOpenMemHandle(&ptr, handle)
                                      // 现在 B 可以访问同一块显存
                                      cudaIpcCloseMemHandle(ptr)
```

## 11. 内存管理策略

- **动态分配**：按需创建 shm / cudaMalloc
- **显式预分配**：`preallocate_*` 接口提前分配常用规格
- **引用计数**：原子操作，归零时回收
- **LRU 缓存**：动态分配的 buffer 可缓存复用
