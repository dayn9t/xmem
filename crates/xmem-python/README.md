# xmem

Cross-process shared memory pool with CPU and CUDA support.

## Installation

```bash
pip install xmem
```

## Quick Start

```python
from xmem import BufferPool

# Create pool
pool = BufferPool("/my_pool")

# Allocate CPU buffer
buf = pool.acquire_cpu(1024)

# Get pointer (can pass to other libraries)
ptr = buf.cpu_ptr

# Set ref count to keep buffer alive
pool.set_ref_count(buf.meta_index, 2)
```

## Features

- **Zero-copy**: Shared memory directly mapped, no data copying
- **Cross-process**: Share memory between different processes
- **CUDA support**: Optional CUDA IPC for GPU memory sharing (install with `xmem[cuda]`)
- **RAII**: Automatic reference counting

## Documentation

Full documentation: https://github.com/dayn9t/xmem

## License

MIT
