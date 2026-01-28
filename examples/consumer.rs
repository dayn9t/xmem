//! 数据消费者 - 打开并读取共享内存
//!
//! 运行此程序打开已存在的共享内存池并读取数据。
//!
//! 使用方法:
//! ```bash
//! cargo run --example consumer
//! ```

use xmem_core::BufferPool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 打开现有池
    let pool = BufferPool::open("/xmem_demo")?;
    println!("Opened pool: {}", pool.name());

    // meta_index 由 producer 确定（通常是 0）
    let meta_index = 0;

    // 读取 buffer
    let buf = pool.get(meta_index)?;
    let data = buf.as_cpu_slice()?;

    println!("Read {} bytes from meta_index={}", data.len(), meta_index);
    println!("Content: {}", std::str::from_utf8(data).unwrap());

    // 显示引用计数
    let rc = pool.ref_count(meta_index)?;
    println!("Reference count: {}", rc);

    Ok(())
}
