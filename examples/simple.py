#!/usr/bin/env python3
"""Python 简单示例"""

from xmem import BufferPool

def main():
    # 创建池
    pool = BufferPool("/xmem_py_demo")
    print(f"Created pool: {pool.name}")

    # 分配 buffer
    buf = pool.acquire_cpu(1024)
    print(f"Buffer meta_index: {buf.meta_index}")
    print(f"Buffer size: {buf.size}")

    # 获取指针（可以传递给其他库）
    ptr = buf.cpu_ptr
    print(f"CPU pointer: {ptr:#x}")

    # 设置引用计数为 2，保持 buffer 存活
    pool.set_ref_count(buf.meta_index, 2)

    # 保持程序运行
    print("\nBuffer is alive. Press Ctrl+C to exit...")
    try:
        input()
    except KeyboardInterrupt:
        pass

if __name__ == "__main__":
    main()
