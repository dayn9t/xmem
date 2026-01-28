# xmem Implementation Plan - Overview

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现一个轻量级跨进程共享内存池库，支持 CPU 共享内存和 CUDA 显存的零拷贝跨进程共享。

**Architecture:** Rust workspace 结构，xmem-core 提供核心功能，xmem-python 提供 Python 绑定。通过 Cargo features 控制 CUDA 支持。使用 POSIX shm 实现 CPU 共享内存，CUDA IPC 实现显存共享。

**Tech Stack:** Rust, shared_memory crate, cudarc (optional), PyO3, maturin

---

## 实施阶段

| 阶段 | 描述 | 文件 |
|------|------|------|
| Phase 1 | 项目骨架 + 基础类型 | [phase1-skeleton.md](./2026-01-28-xmem-impl-phase1.md) |
| Phase 2 | CPU 共享内存实现 | [phase2-cpu-shm.md](./2026-01-28-xmem-impl-phase2.md) |
| Phase 3 | BufferPool + RAII | [phase3-pool.md](./2026-01-28-xmem-impl-phase3.md) |
| Phase 4 | CUDA 支持 | [phase4-cuda.md](./2026-01-28-xmem-impl-phase4.md) |
| Phase 5 | Python 绑定 | [phase5-python.md](./2026-01-28-xmem-impl-phase5.md) |

## 依赖关系

```
Phase 1 (骨架)
    │
    ▼
Phase 2 (CPU shm)
    │
    ▼
Phase 3 (Pool + RAII)
    │
    ├──────────────┐
    ▼              ▼
Phase 4 (CUDA)   Phase 5 (Python)
```

## 测试策略

- 每个模块都有对应的单元测试
- 跨进程测试使用 examples/ 下的 producer/consumer
- TDD：先写测试，再实现

## 验收标准

1. `cargo test` 全部通过
2. `cargo test --features cuda` 全部通过（有 GPU 环境）
3. Python 绑定可正常 import 和使用
4. producer/consumer 示例可跨进程传递数据
