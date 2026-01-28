//! 跨进程集成测试
//!
//! 使用 fork() 创建真正独立的进程来测试跨进程共享内存功能。

#[cfg(all(test, feature = "integration"))]
mod integration {
    use nix::sys::wait::{waitpid, WaitStatus};
    use nix::unistd::{fork, unlink, ForkResult};
    use std::thread;
    use std::time::Duration;

    use xmem_core::BufferPool;

    fn unique_name() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/xmem_test_{}", ts)
    }

    /// 检查 WaitStatus 是否表示成功退出
    fn is_exit_success(status: WaitStatus) -> bool {
        matches!(status, WaitStatus::Exited(_, code) if code == 0)
    }

    /// 测试 CPU 共享内存跨进程读写
    #[test]
    fn test_cpu_cross_process_rw() {
        let name = unique_name();

        match unsafe { fork() }.unwrap() {
           ForkResult::Child => {
                // 子进程：创建池并写入数据
                let pool = BufferPool::create(&name).unwrap();
                let mut buf = pool.acquire_cpu(1024).unwrap();
                let data = b"Hello from child!";
                buf.as_cpu_slice_mut().unwrap()[..data.len()].copy_from_slice(data);
                // 立即sleep，让pool保持打开状态
                thread::sleep(Duration::from_millis(500));
                std::process::exit(0);
            }
            ForkResult::Parent { child } => {
                // 父进程：等待子进程创建池并完成写入

                // 等待并重试打开池（因为子进程需要时间创建）
                let mut attempts = 0;
                let pool = loop {
                    match BufferPool::open(&name) {
                        Ok(p) => break p,
                        Err(e) => {
                            attempts += 1;
                            if attempts > 20 {
                                panic!("Failed to open pool after {} attempts: {:?}", attempts, e);
                            }
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                };

                let buf = pool.get(0).unwrap();
                let read_data = buf.as_cpu_slice().unwrap();
                let expected = b"Hello from child!";
                assert_eq!(&read_data[..expected.len()], expected);

                // 清理
                drop(buf);
                drop(pool);
                clean_shared_memory(&name);

                // 等待子进程退出
                let status = waitpid(child, None).unwrap();
                assert!(is_exit_success(status));
            }
        }
    }

    /// 测试引用计数跨进程传递
    #[test]
    fn test_ref_count_cross_process() {
        let name = unique_name();

        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                let pool = BufferPool::create(&name).unwrap();
                let _buf = pool.acquire_cpu(1024).unwrap();
                pool.set_ref_count(0, 2).unwrap();
                // 子进程结束，buffer 引用计数减 1，还剩 1
                std::process::exit(0);
            }
            ForkResult::Parent { child } => {
                thread::sleep(Duration::from_millis(200));

                // 打开池，引用计数应该为 2（子进程持有 1，这里打开加 1）
                let pool = BufferPool::open(&name).unwrap();
                let rc = pool.ref_count(0).unwrap();
                assert_eq!(rc, 2);

                // 再增加一次
                let rc = pool.add_ref(0).unwrap();
                assert_eq!(rc, 3);

                clean_shared_memory(&name);
                let status = waitpid(child, None).unwrap();
                assert!(is_exit_success(status));
            }
        }
    }

    /// 测试预分配 buffer 跨进程访问
    #[test]
    fn test_preallocated_cross_process() {
        let name = unique_name();

        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                let pool = BufferPool::create(&name).unwrap();
                let indices = pool.preallocate_cpu(512, 3).unwrap();

                // 验证
                assert_eq!(indices.len(), 3);
                assert_eq!(indices, vec![0, 1, 2]);

                std::process::exit(0);
            }
            ForkResult::Parent { child } => {
                thread::sleep(Duration::from_millis(200));

                let pool = BufferPool::open(&name).unwrap();
                let indices = pool.preallocate_cpu(512, 3).unwrap();
                assert_eq!(indices.len(), 3);

                // 验证每个 buffer 的引用计数
                for &idx in &indices {
                    let rc = pool.ref_count(idx).unwrap();
                    assert_eq!(rc, 1);
                }

                clean_shared_memory(&name);
                let _status = waitpid(child, None).unwrap();
            }
        }
    }

    /// 清理共享内存辅助函数
    fn clean_shared_memory(pool_name: &str) {
        // 清理 meta
        let meta_name = format!("{}_meta", pool_name);
        let _ = unlink(meta_name.as_str());

        // 清理 buffers
        for i in 0..10 {
            let buf_name = format!("{}_buf_{}", pool_name, i);
            let _ = unlink(buf_name.as_str());
        }
    }
}
