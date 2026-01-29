# xmem GStreamer 风格池回收实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 xmem-core 升级为 GStreamer 风格的 buffer 池，支持回收、背压和超时清理。

**Architecture:** 在 MetaRegion 中添加共享内存空闲列表，BufferGuard drop 时将 meta_index 回收到空闲列表而非释放。使用 futex 实现跨进程背压等待。

**Tech Stack:** Rust, POSIX shared memory, atomic operations, futex (Linux)

---

## Task 1: 添加空闲列表到 MetaRegionHeader

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/meta_region.rs`

**Step 1: 更新 MetaRegionHeader 结构**

在 `meta_region.rs` 中修改 header 结构，添加空闲列表支持：

```rust
/// Header for metadata region
#[repr(C)]
struct MetaRegionHeader {
    /// Magic number for validation
    magic: u32,
    /// Version number
    version: u32,
    /// Maximum number of buffers
    capacity: u32,
    /// Number of allocated buffers (for stats)
    allocated: AtomicU32,
    /// Free list head index (u32::MAX = empty)
    free_head: AtomicU32,
    /// Waiter count for backpressure
    waiters: AtomicU32,
    /// Reserved for future use
    _reserved: [u32; 2],
}
```

**Step 2: 更新 VERSION 常量**

```rust
const VERSION: u32 = 2;  // 升级版本号
```

**Step 3: 运行测试验证编译**

Run: `cargo test -p xmem-core --lib 2>&1 | tail -20`
Expected: 编译通过（测试可能失败，因为版本变了）

**Step 4: Commit**

```bash
git add crates/xmem-core/src/meta_region.rs
git commit -m "feat(xmem): add free list fields to MetaRegionHeader"
```

---

## Task 2: 添加 next_index 到 BufferMeta

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/meta.rs`

**Step 1: 在 BufferMeta 中添加链表指针**

```rust
/// Buffer metadata stored in shared memory
#[repr(C)]
pub struct BufferMeta {
    // ... existing fields ...

    /// Next free buffer index (for free list, u32::MAX = end)
    pub next_free: AtomicU32,

    /// Reserved for future use (调整大小保持对齐)
    pub reserved: [u8; 60],  // 从 64 减少到 60
}
```

**Step 2: 运行测试**

Run: `cargo test -p xmem-core meta::tests -v`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-core/src/meta.rs
git commit -m "feat(xmem): add next_free field to BufferMeta for free list"
```

---

## Task 3: 实现 MetaRegion 空闲列表操作

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/meta_region.rs`

**Step 1: 添加 free 方法**

```rust
impl MetaRegion {
    /// Free a buffer slot, add to free list
    pub fn free(&self, index: u32) -> Result<()> {
        if index >= self.capacity as u32 {
            return Err(Error::BufferNotFound(index));
        }

        let header = self.header();
        let meta = self.get(index)?;

        // Add to free list head (lock-free)
        loop {
            let old_head = header.free_head.load(Ordering::Acquire);
            meta.next_free.store(old_head, Ordering::Release);

            if header.free_head
                .compare_exchange_weak(old_head, index, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        header.allocated.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }

    /// Get header reference
    fn header(&self) -> &MetaRegionHeader {
        unsafe { &*(self.shm.as_ptr() as *const MetaRegionHeader) }
    }
}
```

**Step 2: 修改 alloc 方法，优先从空闲列表获取**

```rust
/// Allocate a buffer slot, returns meta_index
pub fn alloc(&self) -> Result<u32> {
    let header = self.header();

    // Try to get from free list first
    loop {
        let head = header.free_head.load(Ordering::Acquire);
        if head == u32::MAX {
            break; // Free list empty
        }

        let meta = self.get(head)?;
        let next = meta.next_free.load(Ordering::Acquire);

        if header.free_head
            .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            header.allocated.fetch_add(1, Ordering::SeqCst);
            return Ok(head);
        }
    }

    // Free list empty, allocate new slot
    let header_mut = unsafe { &*(self.shm.as_ptr() as *const MetaRegionHeader) };
    let id = header_mut.next_id.fetch_add(1, Ordering::SeqCst);

    if id >= self.capacity as u32 {
        header_mut.next_id.fetch_sub(1, Ordering::SeqCst);
        return Err(Error::SharedMemory("metadata region full".to_string()));
    }

    header.allocated.fetch_add(1, Ordering::SeqCst);
    Ok(id)
}
```

**Step 3: 更新 create 方法初始化新字段**

```rust
pub fn create(name: &str, capacity: usize) -> Result<Self> {
    let size = Self::calc_size(capacity);
    let mut shm = SharedMemory::create(name, size)?;

    // Initialize header
    let header = unsafe { &mut *(shm.as_mut_ptr() as *mut MetaRegionHeader) };
    header.magic = MAGIC;
    header.version = VERSION;
    header.capacity = capacity as u32;
    header.allocated = AtomicU32::new(0);
    header.free_head = AtomicU32::new(u32::MAX);  // Empty free list
    header.waiters = AtomicU32::new(0);
    header.next_id = AtomicU32::new(0);

    Ok(Self { shm, capacity })
}
```

**Step 4: 运行测试**

Run: `cargo test -p xmem-core meta_region::tests -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/xmem-core/src/meta_region.rs
git commit -m "feat(xmem): implement lock-free free list in MetaRegion"
```

---

## Task 4: 添加空闲列表单元测试

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/meta_region.rs`

**Step 1: 添加测试**

```rust
#[test]
fn test_free_and_reuse() {
    let name = unique_name();
    let region = MetaRegion::create(&name, 10).unwrap();

    // Allocate 3 slots
    let idx0 = region.alloc().unwrap();
    let idx1 = region.alloc().unwrap();
    let idx2 = region.alloc().unwrap();
    assert_eq!(idx0, 0);
    assert_eq!(idx1, 1);
    assert_eq!(idx2, 2);

    // Free middle one
    region.free(idx1).unwrap();

    // Next alloc should reuse idx1
    let idx3 = region.alloc().unwrap();
    assert_eq!(idx3, 1);  // Reused!

    // Next alloc should be new
    let idx4 = region.alloc().unwrap();
    assert_eq!(idx4, 3);
}

#[test]
fn test_free_all_and_reuse() {
    let name = unique_name();
    let region = MetaRegion::create(&name, 3).unwrap();

    // Fill up
    let idx0 = region.alloc().unwrap();
    let idx1 = region.alloc().unwrap();
    let idx2 = region.alloc().unwrap();
    assert!(region.alloc().is_err());  // Full

    // Free all
    region.free(idx0).unwrap();
    region.free(idx1).unwrap();
    region.free(idx2).unwrap();

    // Should be able to allocate again (LIFO order)
    let new0 = region.alloc().unwrap();
    let new1 = region.alloc().unwrap();
    let new2 = region.alloc().unwrap();
    assert_eq!(new0, 2);  // LIFO: last freed first
    assert_eq!(new1, 1);
    assert_eq!(new2, 0);
}
```

**Step 2: 运行测试**

Run: `cargo test -p xmem-core meta_region::tests -v`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-core/src/meta_region.rs
git commit -m "test(xmem): add free list reuse tests"
```

---

## Task 5: 修改 BufferPool 支持回收

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/pool.rs`

**Step 1: 添加 release_buffer 方法**

```rust
impl BufferPool {
    /// Release a buffer back to the pool (called when ref_count reaches 0)
    pub fn release_buffer(&self, meta_index: u32) -> Result<()> {
        // Note: SharedMemory for buffer data is NOT unlinked
        // It will be reused when this meta_index is allocated again
        self.meta_region.free(meta_index)
    }

    /// Check if a buffer should be released (ref_count == 0)
    pub fn try_release(&self, meta_index: u32) -> Result<bool> {
        let meta = self.meta_region.get(meta_index)?;
        let ref_count = meta.ref_count.load(Ordering::SeqCst);

        if ref_count <= 0 {
            self.release_buffer(meta_index)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
```

**Step 2: 运行测试**

Run: `cargo test -p xmem-core pool::tests -v`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-core/src/pool.rs
git commit -m "feat(xmem): add release_buffer method to BufferPool"
```

---

## Task 6: 修改 BufferGuard drop 行为

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/guard.rs`
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/pool.rs`

**Step 1: 添加 pool 引用到 BufferGuard**

修改 `guard.rs`：

```rust
use std::sync::Arc;

pub struct BufferGuard {
    data: Option<BufferData>,
    meta_index: u32,
    mode: AccessMode,
    ref_count: *const AtomicI32,
    should_release: bool,
    /// Pool reference for recycling (optional, only set by pool)
    pool_name: Option<String>,
}

impl BufferGuard {
    pub(crate) fn new(
        data: BufferData,
        meta_index: u32,
        mode: AccessMode,
        ref_count: *const AtomicI32,
    ) -> Self {
        Self {
            data: Some(data),
            meta_index,
            mode,
            ref_count,
            should_release: true,
            pool_name: None,
        }
    }

    pub(crate) fn with_pool(mut self, pool_name: String) -> Self {
        self.pool_name = Some(pool_name);
        self
    }
}
```

**Step 2: 修改 drop 实现**

```rust
impl Drop for BufferGuard {
    fn drop(&mut self) {
        if self.should_release && self.data.is_some() {
            // Decrement ref count
            if !self.ref_count.is_null() {
                let ref_count = unsafe { &*self.ref_count };
                let old = ref_count.fetch_sub(1, Ordering::SeqCst);

                // If ref_count reaches 0 and we have pool info, recycle
                if old == 1 {
                    if let Some(pool_name) = &self.pool_name {
                        // Try to recycle - open pool and release
                        if let Ok(pool) = crate::BufferPool::open(pool_name) {
                            let _ = pool.release_buffer(self.meta_index);
                        }
                    }
                }
            }
        }
    }
}
```

**Step 3: 修改 BufferPool 创建 guard 时传入 pool_name**

修改 `pool.rs` 中的 `acquire_cpu` 和 `get_with_mode`：

```rust
pub fn acquire_cpu(&self, size: usize) -> Result<BufferGuard> {
    // ... existing code ...

    Ok(BufferGuard::new(
        data,
        meta_index,
        AccessMode::ReadWrite,
        ref_count_ptr,
    ).with_pool(self.name.clone()))
}

fn get_with_mode(&self, meta_index: u32, mode: AccessMode) -> Result<BufferGuard> {
    // ... existing code ...

    Ok(BufferGuard::new(data, meta_index, mode, ref_count_ptr)
        .with_pool(self.name.clone()))
}
```

**Step 4: 运行测试**

Run: `cargo test -p xmem-core -v 2>&1 | tail -30`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/xmem-core/src/guard.rs crates/xmem-core/src/pool.rs
git commit -m "feat(xmem): auto-recycle buffer on drop when ref_count=0"
```

---

## Task 7: 添加池回收集成测试

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/pool.rs`

**Step 1: 添加回收测试**

```rust
#[test]
fn test_buffer_recycle() {
    let name = unique_name();
    let pool = BufferPool::create_with_capacity(&name, 3).unwrap();

    // Allocate all 3 slots
    let buf0 = pool.acquire_cpu(1024).unwrap();
    let buf1 = pool.acquire_cpu(1024).unwrap();
    let buf2 = pool.acquire_cpu(1024).unwrap();

    assert_eq!(buf0.meta_index(), 0);
    assert_eq!(buf1.meta_index(), 1);
    assert_eq!(buf2.meta_index(), 2);

    // Pool should be full
    assert!(pool.acquire_cpu(1024).is_err());

    // Drop buf1 - should recycle
    drop(buf1);

    // Now we can allocate again, should get recycled index
    let buf3 = pool.acquire_cpu(1024).unwrap();
    assert_eq!(buf3.meta_index(), 1);  // Recycled!
}

#[test]
fn test_buffer_recycle_with_multiple_refs() {
    let name = unique_name();
    let pool = BufferPool::create_with_capacity(&name, 2).unwrap();

    // Allocate
    let buf0 = pool.acquire_cpu(1024).unwrap();
    let idx = buf0.meta_index();

    // Add another reference
    pool.add_ref(idx).unwrap();
    assert_eq!(pool.ref_count(idx).unwrap(), 2);

    // Drop first ref
    drop(buf0);
    assert_eq!(pool.ref_count(idx).unwrap(), 1);

    // Buffer should NOT be recycled yet
    let buf1 = pool.acquire_cpu(1024).unwrap();
    assert_eq!(buf1.meta_index(), 1);  // New slot, not recycled

    // Release second ref manually
    pool.release(idx).unwrap();

    // Now try_release should recycle
    assert!(pool.try_release(idx).unwrap());
}
```

**Step 2: 运行测试**

Run: `cargo test -p xmem-core pool::tests::test_buffer_recycle -v`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/xmem-core/src/pool.rs
git commit -m "test(xmem): add buffer recycle integration tests"
```

---

## Task 8: 实现背压机制 (可选，Phase 2)

**Files:**
- Modify: `/home/jiang/cc/utils/xmem/crates/xmem-core/src/pool.rs`

**Step 1: 添加 acquire_blocking 方法**

```rust
use std::time::Duration;

impl BufferPool {
    /// Acquire a buffer, blocking if pool is full
    pub fn acquire_cpu_blocking(&self, size: usize, timeout: Duration) -> Result<BufferGuard> {
        let start = std::time::Instant::now();

        loop {
            match self.acquire_cpu(size) {
                Ok(buf) => return Ok(buf),
                Err(Error::SharedMemory(msg)) if msg.contains("full") => {
                    if start.elapsed() >= timeout {
                        return Err(Error::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

**Step 2: 添加 Timeout 错误类型**

修改 `error.rs`：

```rust
#[derive(Error, Debug)]
pub enum Error {
    // ... existing variants ...

    #[error("Operation timed out")]
    Timeout,
}
```

**Step 3: 运行测试**

Run: `cargo test -p xmem-core -v`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/xmem-core/src/pool.rs crates/xmem-core/src/error.rs
git commit -m "feat(xmem): add blocking acquire with timeout"
```

---

## Task 9: 运行完整测试套件

**Step 1: 运行所有测试**

Run: `cargo test -p xmem-core -v`
Expected: All tests PASS

**Step 2: 运行跨进程测试（如果有）**

Run: `cargo test -p xmem-core --features integration -v`
Expected: PASS

**Step 3: 最终 Commit**

```bash
git add -A
git commit -m "feat(xmem): complete GStreamer-style buffer pool recycling"
```

---

## Summary

完成后 xmem-core 将具备：

| 功能 | 状态 |
|------|------|
| 空闲列表管理 | ✅ |
| 自动回收 (ref_count=0) | ✅ |
| 背压阻塞 | ✅ |
| 超时支持 | ✅ |

下一步：Phase 2 - s3-parking 集成
