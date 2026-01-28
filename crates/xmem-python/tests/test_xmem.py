"""Tests for xmem Python bindings."""

import pytest
import time


def unique_name():
    """Generate unique pool name."""
    return f"/xmem_pytest_{int(time.time() * 1e9)}"


class TestBufferPool:
    """Tests for BufferPool."""

    def test_create_pool(self):
        """Test creating a new pool."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)
        assert pool.name == name
        assert pool.capacity == 1024

    def test_create_with_capacity(self):
        """Test creating pool with custom capacity."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name, capacity=100)
        assert pool.capacity == 100

    def test_acquire_cpu(self):
        """Test acquiring CPU buffer."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        assert buf.meta_index == 0
        assert buf.is_valid
        assert buf.size == 1024

    def test_ref_count(self):
        """Test reference counting."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        meta_index = buf.meta_index

        assert pool.ref_count(meta_index) == 1

        pool.add_ref(meta_index)
        assert pool.ref_count(meta_index) == 2

        pool.release(meta_index)
        assert pool.ref_count(meta_index) == 1

    def test_forget(self):
        """Test forget without release."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        meta_index = buf.meta_index
        buf.forget()

        # Ref count should still be 1
        assert pool.ref_count(meta_index) == 1

    def test_context_manager(self):
        """Test context manager usage."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        meta_index = None
        with pool.acquire_cpu(1024) as buf:
            meta_index = buf.meta_index
            pool.set_ref_count(meta_index, 2)
            assert buf.is_valid

        # After context, ref count should be decremented
        assert pool.ref_count(meta_index) == 1

    def test_preallocate_cpu(self):
        """Test preallocating CPU buffers."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        indices = pool.preallocate_cpu(1024, 5)
        assert len(indices) == 5

        for idx in indices:
            assert pool.ref_count(idx) == 1


class TestBufferGuard:
    """Tests for BufferGuard."""

    def test_cpu_ptr(self):
        """Test getting CPU pointer."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        ptr = buf.cpu_ptr
        assert ptr > 0

    def test_read_only_guard(self):
        """Test read-only guard."""
        from xmem import BufferPool

        name = unique_name()
        pool = BufferPool(name)

        buf = pool.acquire_cpu(1024)
        meta_index = buf.meta_index
        pool.set_ref_count(meta_index, 2)
        buf.forget()

        # Get read-only
        buf = pool.get(meta_index)
        _ = buf.cpu_ptr  # Should work

        with pytest.raises(RuntimeError):
            _ = buf.cpu_ptr_mut  # Should fail
