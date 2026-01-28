# xmem - 跨进程共享内存池

## 概述

xmem 是一个轻量级跨进程共享内存池库，支持 CPU 共享内存和 CUDA 显存的零拷贝跨进程共享。

## 核心特性

- **CPU 共享内存**：基于 POSIX shm，零拷贝跨进程访问
- **CUDA 显存共享**：基于 CUDA IPC，跨进程直接访问 GPU 内存（可选 feature）
- **CPU/CUDA 互操作**：支持 CPU ↔ CUDA、CUDA ↔ CUDA 数据复制
- **引用计数**：原子操作，支持多消费者场景
- **RAII 封装**：Rust 侧自动管理生命周期
- **框架兼容**：与 PyTorch、ONNX Runtime 零拷贝集成
- **跨语言**：Rust 核心 + Python 绑定 (PyO3)
- **条件编译**：通过 Cargo features 控制 CUDA 支持

## 架构

```
┌─────────────────────────────────────────────────────────┐
│              Shared Metadata Region                     │
│           (存放所有 buffer 元数据 + 引用计数)             │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│                   Buffer Pool                           │
│         CPU (POSIX shm) + GPU (CUDA IPC)               │
└─────────────────────────────────────────────────────────┘
```

## Cargo Features

| Feature | 说明 | 默认 |
|---------|------|------|
| `cpu` | CPU 共享内存支持 | ✓ |
| `cuda` | CUDA 显存 + IPC 支持 | - |

```bash
# 仅 CPU
cargo build

# CPU + CUDA
cargo build --features cuda
```

## 快速示例

### Rust

```rust
use xmem::BufferPool;

let pool = BufferPool::new("my_pool")?;

// 生产者
let mut buf = pool.acquire_cpu(1920 * 1080 * 3)?;
write_data(buf.as_cpu_slice_mut()?);
let meta_index = buf.meta_index();
std::mem::forget(buf);  // 转移所有权

// 消费者
let buf = pool.get(meta_index)?;
process(buf.as_cpu_slice()?);
// drop 时自动 release
```

### Python

```python
from xmem import BufferPool

pool = BufferPool("my_pool")

# 生产者
buf = pool.acquire_cpu(1920 * 1080 * 3)
write_data(buf.cpu_ptr)
meta_index = buf.meta_index
buf.forget()

# 消费者
buf = pool.get(meta_index)
process(buf.cpu_ptr)
```

## 使用场景

- 本地微服务间传递大块数据（图像、tensor）
- 类 GStreamer 的 pipeline 架构
- ML 推理流水线（decoder → filter → encoder）
- 任何需要高效跨进程数据共享的场景

## 文档导航

- [技术设计详情](./2026-01-28-xmem-design.md)
