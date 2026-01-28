//! 数据生产者 - 分配并写入共享内存
//!
//! 运行此程序分配共享内存并写入数据，
//! 然后保持程序运行以便 consumer 读取。
//!
//! 使用方法:
//! ```bash
//! cargo run --example producer
//! ```

use xmem_core::BufferPool;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建池
    let pool = BufferPool::create("/xmem_demo")?;
    println!("Created pool: {}", pool.name());

    // 分配 buffer
    let mut buf = pool.acquire_cpu(1024)?;
    let data = b"Hello from producer! This is shared memory.";
    buf.as_cpu_slice_mut()?.copy_from_slice(data);

    let idx = buf.meta_index();
    println!("Written {} bytes at meta_index={}", data.len(), idx);
    println!("Buffer size: {}", buf.size);

    // 保持 buffer 存活，供 consumer 读取
    println!("\nBuffer is alive. Press Ctrl+C to exit...");

    // 设置引用计数为 2，这样即使这个 guard drop，buffer 仍然存在
    pool.set_ref_count(idx, 2)?;

    // 保持程序运行
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
