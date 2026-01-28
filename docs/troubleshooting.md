# 故障排查

## 常见问题

### Shared memory 残留

**症状**: `/dev/shm/` 下残留 xmem_* 文件

**原因**: 程序异常退出，未正确清理共享内存

**解决**:
```bash
# 查看残留文件
ls /dev/shm/xmem_*

# 清理特定池的所有文件
POOL_NAME="/xmem_demo"
rm /dev/shm/${POOL_NAME}_meta
rm /dev/shm/${POOL_NAME}_buf_*

# 清理所有 xmem 文件（谨慎！）
rm /dev/shm/xmem_*_*
```

### Buffer not found

**症状**: `Error::BufferNotFound(0)`

**原因**: meta_index 不存在或已被释放

**解决**:
- 确保 producer 先运行并分配了 buffer
- 检查 meta_index 是否正确

### Permission denied

**症状**: `Error::SharedMemory("Permission denied")`

**原因**: 共享内存权限问题

**解决**:
```bash
# 检查共享内存权限
ls -l /dev/shm/xmem_*

# 确保用户有读写权限
chmod 660 /dev/shm/xmem_*
```

### CUDA 相关

#### CUDA 初始化失败

**症状**: `CUDA_ERROR_NOT_INITIALIZED`

**解决**:
```bash
# 检查 CUDA 环境
nvidia-smi

# 设置 CUDA_VISIBLE_DEVICES
export CUDA_VISIBLE_DEVICES=0

# 检查驱动
cat /proc/driver/nvidia/version
```

#### CUDA IPC 失败

**症状**: `cuIpcOpenMemHandle failed`

**原因**:
- GPU 不支持 peer access
- 设备间没有启用 P2P

**解决**:
```bash
# 启用 P2P（需要 root 权限
nvidia-smi -i 0 -c 0

# 或使用同一 GPU
device_id = 0
```

### Python 相关

#### 导入失败

**症状**: `ImportError: cannot import name 'xmem'`

**原因**: Python 包未构建

**解决**:
```bash
cd crates/xmem-python
maturin develop --release
```

#### 运行时错误

**症状**: `RuntimeError: buffer already forgotten`

**原因**: 在 forget() 后访问 buffer

**解决**:
```python
buf = pool.acquire_cpu(1024)
buf.forget()  # 转移所有权后不能再访问

# 正确做法：
# 不要在 forget 后访问，或重新获取
buf = pool.get(buf.meta_index)
```

## 调试技巧

### 启用日志

设置环境变量启用详细日志：

```bash
RUST_LOG=debug cargo run
```

### 查看共享内存

```bash
# 查看所有共享内存
ipcs -m

# 查看详细信息
ipcs -m -i <shmid>

# 查看进程 attachment
lsof | grep "/xmem_"
```

### 检查引用计数

```rust
let rc = pool.ref_count(meta_index)?;
println!("Ref count: {}", rc);
```
