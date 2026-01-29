# xmem v0.1.0

首个正式版本发布！🎉

## ✨ 特性

- ✅ **跨进程共享内存池**: 基于 POSIX 共享内存的零拷贝数据传输
- ✅ **CPU 和 CUDA 支持**: 可选的 CUDA IPC 支持，实现 GPU 内存跨进程共享
- ✅ **RAII 风格引用计数**: 自动管理缓冲区生命周期
- ✅ **Python 绑定**: 通过 PyO3 提供 Python 接口
- ✅ **类型安全**: 完整的 Rust 类型系统保证

## 🚀 性能

基于 criterion 的基准测试结果：

| 操作 | 性能 |
|------|------|
| 池创建 | 6.67 µs |
| 池打开 | 3.53 µs |
| 缓冲区分配 | ~9 µs (与大小无关) |
| 读写操作 | 122 ns - 8 µs |
| 吞吐量 | 7-8 GiB/s (小缓冲区), 100+ GiB/s (大缓冲区) |
| 引用计数操作 | ~7 ns |

详细性能报告: [benchmark-results.md](https://github.com/dayn9t/xmem/blob/master/docs/benchmark-results.md)

## 📦 安装

### Rust

```bash
cargo add xmem-core
```

或在 `Cargo.toml` 中添加：

```toml
[dependencies]
xmem-core = "0.1.0"

# 可选：启用 CUDA 支持
xmem-core = { version = "0.1.0", features = ["cuda"] }
```

### Python

```bash
pip install xmem
```

## 📖 快速开始

### Rust

```rust
use xmem_core::BufferPool;

// 创建池
let pool = BufferPool::create("/my_pool")?;

// 分配 CPU 缓冲区
let mut buf = pool.acquire_cpu(1024)?;

// 写入数据
let data = b"Hello, xmem!";
buf.as_cpu_slice_mut()?[..data.len()].copy_from_slice(data);

// 获取索引，传递给其他进程
let idx = buf.meta_index();
```

### Python

```python
from xmem import BufferPool

# 创建池
pool = BufferPool("/my_pool")

# 分配缓冲区
buf = pool.acquire_cpu(1024)

# 获取指针（可传递给 NumPy、PyTorch 等）
ptr = buf.cpu_ptr
```

## 📚 文档

- [README](https://github.com/dayn9t/xmem)
- [API 文档](https://docs.rs/xmem-core)
- [性能报告](https://github.com/dayn9t/xmem/blob/master/docs/benchmark-results.md)
- [故障排除指南](https://github.com/dayn9t/xmem/blob/master/docs/troubleshooting.md)

## 🧪 测试

- ✅ 15 个单元测试
- ✅ 3 个跨进程集成测试
- ✅ 9 个文档测试
- ✅ 性能基准测试

## 🔧 技术栈

- **Rust**: 核心库实现
- **PyO3**: Python 绑定
- **POSIX 共享内存**: 跨进程通信
- **CUDA IPC**: GPU 内存共享（可选）
- **Criterion**: 性能基准测试

## 🎯 适用场景

- 跨进程大数据传输（视频、图像、深度学习）
- 高频小数据交换（IPC 消息队列）
- 多进程并行计算（数据共享）
- GPU-CPU 数据交换（CUDA IPC）

## 📝 更新日志

### 新增
- 核心共享内存池实现
- CPU 缓冲区分配和管理
- CUDA IPC 支持（可选）
- Python 绑定
- 跨进程集成测试
- 性能基准测试
- 完整文档

## 🙏 致谢

感谢所有贡献者和测试者！

## 📄 许可证

MIT License

---

**完整更新日志**: https://github.com/dayn9t/xmem/commits/v0.1.0
