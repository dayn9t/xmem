//! RAII buffer guard
//!
//! 提供 [`BufferGuard`] 类型用于 RAII 风格的缓冲区访问管理。
//!
//! # 示例
//!
//! ```
//! use xmem_core::BufferPool;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = BufferPool::create("/my_pool")?;
//!
//! {
//!     let mut buf = pool.acquire_cpu(1024)?;
//!     buf.as_cpu_slice_mut()?.copy_from_slice(b"hello");
//!     // Drop 时自动释放引用
//! }
//! # Ok(())
//! # }
//! ```

use crate::buffer::BufferData;
use crate::storage::AccessMode;
use crate::{Error, Result};
use std::sync::atomic::{AtomicI32, Ordering};

/// RAII 风格的缓冲区访问守卫
///
/// 自动管理缓冲区引用计数，在 Drop 时递减引用。
///
/// # 示例
///
/// ```
/// use xmem_core::BufferPool;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = BufferPool::create("/my_pool")?;
///
/// // 分配 buffer
/// let mut buf = pool.acquire_cpu(1024)?;
/// let idx = buf.meta_index();
///
/// // 写入数据
/// buf.as_cpu_slice_mut()?.copy_from_slice(b"hello");
///
/// // 传递所有权给其他进程
/// buf.forget();
/// # Ok(())
/// # }
/// ```
///
/// # 生命周期
///
/// [`BufferGuard`] 在 Drop 时会自动递减引用计数。
/// 使用 [`forget()`] 方法可以转移所有权而不释放引用。
///
/// [`forget()`]: Self::forget
pub struct BufferGuard {
    /// Buffer data
    data: Option<BufferData>,
    /// Metadata index
    meta_index: u32,
    /// Access mode
    mode: AccessMode,
    /// Reference to ref_count in shared memory
    ref_count: *const AtomicI32,
    /// Whether this guard owns the release responsibility
    should_release: bool,
}

// Safety: BufferGuard can be sent between threads
// The underlying shared memory is process-wide accessible
unsafe impl Send for BufferGuard {}

impl BufferGuard {
    /// 创建新的缓冲区守卫（内部使用）
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
        }
    }

    /// 获取元数据索引
    ///
    /// 返回此 guard 管理的 buffer 的元数据索引，
    /// 可用于在其他进程中打开同一 buffer。
    pub fn meta_index(&self) -> u32 {
        self.meta_index
    }

    /// 获取访问模式
    pub fn mode(&self) -> AccessMode {
        self.mode
    }

    /// 检查 buffer 是否仍然有效
    ///
    /// 如果已调用 [`forget()`]，返回 `false`。
    ///
    /// [`forget()`]: Self::forget
    pub fn is_valid(&self) -> bool {
        self.data.is_some()
    }

    /// 获取 CPU 只读切片
    ///
    /// # 错误
    ///
    /// - [`Error::AlreadyForgotten`]: guard 已被 forget
    /// - [`Error::TypeMismatch`]: buffer 不是 CPU 类型
    pub fn as_cpu_slice(&self) -> Result<&[u8]> {
        let data = self.data.as_ref().ok_or(Error::AlreadyForgotten)?;
        let ptr = data.as_cpu_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cpu".to_string(),
            actual: "Cuda".to_string(),
        })?;
        Ok(unsafe { std::slice::from_raw_parts(ptr, data.size()) })
    }

    /// Get CPU slice (mutable, requires ReadWrite mode)
    pub fn as_cpu_slice_mut(&mut self) -> Result<&mut [u8]> {
        if self.mode == AccessMode::ReadOnly {
            return Err(Error::ReadOnly);
        }
        let data = self.data.as_mut().ok_or(Error::AlreadyForgotten)?;
        let size = data.size();
        let ptr = data.as_cpu_mut_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cpu".to_string(),
            actual: "Cuda".to_string(),
        })?;
        Ok(unsafe { std::slice::from_raw_parts_mut(ptr, size) })
    }

    /// Get CUDA device pointer (read-only)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr(&self) -> Result<u64> {
        let data = self.data.as_ref().ok_or(Error::AlreadyForgotten)?;
        data.as_cuda_ptr().ok_or_else(|| Error::TypeMismatch {
            expected: "Cuda".to_string(),
            actual: "Cpu".to_string(),
        })
    }

    /// Get CUDA device pointer (mutable, requires ReadWrite mode)
    #[cfg(feature = "cuda")]
    pub fn as_cuda_ptr_mut(&mut self) -> Result<u64> {
        if self.mode == AccessMode::ReadOnly {
            return Err(Error::ReadOnly);
        }
        self.as_cuda_ptr()
    }

    /// Forget this guard without releasing the buffer
    /// Used when transferring ownership to another process
    pub fn forget(mut self) {
        self.should_release = false;
        self.data = None;
    }

    /// Manually release and decrement ref count
    fn release(&self) {
        if !self.ref_count.is_null() {
            let ref_count = unsafe { &*self.ref_count };
            ref_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for BufferGuard {
    fn drop(&mut self) {
        if self.should_release && self.data.is_some() {
            self.release();
        }
    }
}
