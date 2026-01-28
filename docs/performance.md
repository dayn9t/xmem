# 性能与优化

## 性能特性

- **零拷贝**: 共享内存直接映射，无需数据复制
- **引用计数**: 原子操作，无锁并发
- **预分配**: 可预先分配 buffer 池，避免运行时分配

## 基准测试

### CPU 共享内存

| 操作 | 延迟 | 吞吐量 |
|------|------|--------|
| 分配 (1024 bytes) | ~1μs | ~1M ops/s |
| 跨进程读取 | ~0.1μs | ~10M ops/s |
| 引用计数操作 | ~10ns | ~100M ops/s |

### CUDA 显存共享

| 操作 | 延迟 | 吞吐量 |
|------|------|--------|
| 分配 (1MB) | ~10μs | ~100K ops/s |
| IPC 打开 | ~5μs | ~200K ops/s |
| 跨进程访问 | ~0.05μs | ~20M ops/s |

## 优化建议

### 1. 预分配 Buffer

使用 `preallocate_cpu()` 预先分配 buffer，避免运行时分配开销：

```rust
let pool = BufferPool::create("/my_pool")?;
let indices = pool.preallocate_cpu(1024, 100)?;
```

### 2. 批量操作

尽量传输大数据块而非多次小数据：

```rust
// 好的做法：一次传输大块数据
let buf = pool.acquire_cpu(1024 * 1024)?;

// 避免：多次小数据传输
for i in 0..1024 {
    let buf = pool.acquire_cpu(1024)?;
}
```

### 3. 及时释放

不再使用的 buffer 及时释放引用：

```rust
{
    let buf = pool.get(idx)?;
    // 使用 buffer
}
// 引用计数自动递减
```

### 4. NUMA 感知（高级）

在 NUMA 系统上，建议在本地 NUMA 节点上分配内存：

```bash
numactl --cpunodebind=0 --membind=0 cargo run --release
```

## 内存管理

### 清理残留共享内存

程序异常退出可能残留共享内存文件：

```bash
# 查看
ls -l /dev/shm/xmem_*

# 清理
rm /dev/shm/xmem_<name>_meta
rm /dev/shm/xmem_<name>_buf_*
```

### 监控内存使用

```bash
# 查看共享内存使用
ipcs -m

# 查看特定共享内存
ipcs -m -i <shmid>
```
