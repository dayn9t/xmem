# xmem

跨进程共享内存池，支持 CPU 共享内存和 CUDA 显存的零拷贝跨进程共享。

## 特性

- **零拷贝跨进程共享** - 直接内存映射，无需数据复制
- **CPU 共享内存** - 基于 POSIX shm 的跨进程共享
- **CUDA 显存共享** - 基于 CUDA IPC 的 GPU 显存零拷贝共享
- **RAII 自动管理** - 自动资源管理和引用计数
- **双语言支持** - Rust 和 Python API

## 安装

### Rust

```toml
[dependencies]
xmem-core = "0.1"
```

启用 CUDA 支持：

```toml
[dependencies]
xmem-core = { version = "0.1", features = ["cuda"] }
```

### Python

```bash
pip install xmem
```

## 快速开始

### Rust

```rust
use xmem_core::BufferPool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建共享内存池
    let pool = BufferPool::create("/my_pool")?;

    // 分配 buffer
    let mut buf = pool.acquire_cpu(1024)?;
    buf.as_cpu_slice_mut()?.copy_from_slice(b"hello world");

    // 将 meta_index 传递给其他进程
    let idx = buf.meta_index();
    println!("Buffer created at meta_index={}", idx);

    Ok(())
}
```

### Python

```python
from xmem import BufferPool

# 创建池
pool = BufferPool("/my_pool")

# 分配 buffer
buf = pool.acquire_cpu(1024)
print(f"Buffer created at meta_index={buf.meta_index}")

# 传递 meta_index 给其他进程
```

## 文档

- [Rust API 文档](https://docs.rs/xmem-core)
- [示例代码](./examples/)

## 许可证

MIT License
