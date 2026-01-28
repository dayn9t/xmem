"""Type stubs for xmem Python bindings."""

from typing import List

class BufferPool:
    """Cross-process shared memory buffer pool."""

    def __init__(self, name: str, capacity: int = 1024) -> None:
        """Create a new buffer pool."""
        ...

    @staticmethod
    def open(name: str) -> "BufferPool":
        """Open an existing buffer pool."""
        ...

    @property
    def name(self) -> str:
        """Get pool name."""
        ...

    @property
    def capacity(self) -> int:
        """Get pool capacity."""
        ...

    def acquire_cpu(self, size: int) -> "BufferGuard":
        """Acquire a CPU buffer."""
        ...

    def acquire_cuda(self, size: int, device_id: int) -> "BufferGuard":
        """Acquire a CUDA buffer (requires cuda feature)."""
        ...

    def preallocate_cpu(self, size: int, count: int) -> List[int]:
        """Preallocate CPU buffers."""
        ...

    def preallocate_cuda(self, size: int, count: int, device_id: int) -> List[int]:
        """Preallocate CUDA buffers (requires cuda feature)."""
        ...

    def get(self, meta_index: int) -> "BufferGuard":
        """Get a buffer (read-only)."""
        ...

    def get_mut(self, meta_index: int) -> "BufferGuard":
        """Get a buffer (read-write)."""
        ...

    def set_ref_count(self, meta_index: int, count: int) -> None:
        """Set reference count."""
        ...

    def add_ref(self, meta_index: int) -> int:
        """Add reference, returns new count."""
        ...

    def release(self, meta_index: int) -> int:
        """Release reference, returns new count."""
        ...

    def ref_count(self, meta_index: int) -> int:
        """Get current reference count."""
        ...


class BufferGuard:
    """RAII guard for buffer access."""

    @property
    def meta_index(self) -> int:
        """Get metadata index."""
        ...

    @property
    def is_valid(self) -> bool:
        """Check if buffer is valid."""
        ...

    @property
    def cpu_ptr(self) -> int:
        """Get CPU pointer (read-only)."""
        ...

    @property
    def cpu_ptr_mut(self) -> int:
        """Get CPU pointer (read-write)."""
        ...

    @property
    def cuda_ptr(self) -> int:
        """Get CUDA device pointer (read-only, requires cuda feature)."""
        ...

    @property
    def cuda_ptr_mut(self) -> int:
        """Get CUDA device pointer (read-write, requires cuda feature)."""
        ...

    @property
    def size(self) -> int:
        """Get buffer size in bytes."""
        ...

    def forget(self) -> None:
        """Forget this guard without releasing the buffer."""
        ...

    def __enter__(self) -> "BufferGuard":
        """Context manager enter."""
        ...

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        """Context manager exit."""
        ...
