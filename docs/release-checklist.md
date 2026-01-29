# xmem 发布检查清单

## 发布前检查

### 代码质量
- [x] 所有测试通过 (27/27)
- [x] 无编译警告
- [x] 代码审查完成
- [x] 文档完整

### 版本信息
- [x] 版本号: 0.1.0
- [x] Changelog 更新
- [x] Git 标签准备

### 依赖检查
- [x] 依赖版本固定
- [x] 可选依赖正确配置
- [x] 无循环依赖

## crates.io 发布

### 准备工作
```bash
# 1. 登录 crates.io
cargo login
# 输入你的 API token (从 https://crates.io/me 获取)

# 2. 验证包
cd crates/xmem-core
cargo publish --dry-run

# 3. 发布
cargo publish
```

### 发布后验证
```bash
# 等待几分钟后
cargo search xmem-core
cargo install xmem-core --version 0.1.0
```

## PyPI 发布

### 准备工作
```bash
# 1. 安装 maturin
pip install --user maturin[patchelf]
# 或
cargo install maturin

# 2. 配置 PyPI token
# 在 ~/.pypirc 添加:
# [pypi]
# username = __token__
# password = pypi-...

# 3. 构建测试
cd crates/xmem-python
maturin build --release

# 4. 本地测试
pip install --force-reinstall target/wheels/*.whl
python -c "import xmem; print(xmem.__version__)"
```

### 发布
```bash
cd crates/xmem-python
maturin publish
```

### 发布后验证
```bash
# 等待几分钟后
pip install xmem
python -c "import xmem; print(xmem.__version__)"
```

## Git 标签

```bash
# 创建标签
git tag -a v0.1.0 -m "Release v0.1.0"

# 推送标签
git push origin v0.1.0
```

## 发布公告

### GitHub Release
1. 访问 https://github.com/dayn9t/xmem/releases/new
2. 选择标签 v0.1.0
3. 标题: xmem v0.1.0
4. 内容:
```markdown
# xmem v0.1.0

首个正式版本发布！

## 特性

- ✅ 跨进程共享内存池
- ✅ CPU 和 CUDA 支持
- ✅ RAII 风格的引用计数
- ✅ Python 绑定
- ✅ 零拷贝数据传输

## 性能

- 池创建: 6.67µs
- 缓冲区分配: ~9µs
- 读写: 122ns - 8µs (7-8 GiB/s)
- 引用计数: ~7ns

## 安装

**Rust:**
```bash
cargo add xmem-core
```

**Python:**
```bash
pip install xmem
```

## 文档

- [README](https://github.com/dayn9t/xmem)
- [API 文档](https://docs.rs/xmem-core)
- [性能报告](https://github.com/dayn9t/xmem/blob/master/docs/benchmark-results.md)
```

## 社区通知

### Reddit
- r/rust
- r/Python

### Twitter/X
```
🚀 xmem v0.1.0 发布！

跨进程共享内存池，支持 CPU 和 CUDA
- 零拷贝
- 低延迟 (~9µs)
- 高吞吐 (100+ GiB/s)

Rust + Python 绑定

https://github.com/dayn9t/xmem
#rustlang #python #ipc
```

## 检查清单

- [ ] crates.io 发布成功
- [ ] PyPI 发布成功
- [ ] Git 标签创建
- [ ] GitHub Release 创建
- [ ] 文档链接验证
- [ ] 社区通知发送
