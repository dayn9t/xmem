//! Performance benchmarks for xmem
//!
//! Run with: cargo bench --package xmem-core

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use xmem_core::BufferPool;
use std::time::SystemTime;

fn unique_name() -> String {
    let ts = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("/xmem_bench_{}", ts)
}

fn bench_pool_create(c: &mut Criterion) {
    c.bench_function("pool_create", |b| {
        b.iter(|| {
            let name = unique_name();
            let pool = BufferPool::create(&name).unwrap();
            black_box(pool);
        });
    });
}

fn bench_buffer_acquire(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_acquire");
    group.sample_size(50); // 减少样本数避免元数据区域满

    for size in [1024, 4096, 65536, 1048576].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let name = unique_name();
                    BufferPool::create(&name).unwrap()
                },
                |pool| {
                    let buf = pool.acquire_cpu(size).unwrap();
                    black_box(buf);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_buffer_write_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_write_read");
    group.sample_size(50);

    for size in [1024, 4096, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let name = unique_name();
            let pool = BufferPool::create(&name).unwrap();
            let mut buf = pool.acquire_cpu(size).unwrap();
            let data = vec![42u8; size];

            b.iter(|| {
                // Write
                let slice = buf.as_cpu_slice_mut().unwrap();
                slice[..data.len()].copy_from_slice(&data);

                // Read
                let slice = buf.as_cpu_slice().unwrap();
                let sum: u64 = slice.iter().map(|&x| x as u64).sum();
                black_box(sum);
            });
        });
    }
    group.finish();
}

fn bench_ref_count_ops(c: &mut Criterion) {
    let name = unique_name();
    let pool = BufferPool::create(&name).unwrap();
    let _buf = pool.acquire_cpu(1024).unwrap();

    let mut group = c.benchmark_group("ref_count");

    group.bench_function("add_ref", |b| {
        b.iter(|| {
            let rc = pool.add_ref(0).unwrap();
            black_box(rc);
        });
    });

    group.bench_function("release", |b| {
        b.iter(|| {
            let rc = pool.release(0).unwrap();
            black_box(rc);
        });
    });

    group.bench_function("get_ref_count", |b| {
        b.iter(|| {
            let rc = pool.ref_count(0).unwrap();
            black_box(rc);
        });
    });

    group.finish();
}

fn bench_pool_open(c: &mut Criterion) {
    let name = unique_name();
    let _pool = BufferPool::create(&name).unwrap();

    c.bench_function("pool_open", |b| {
        b.iter(|| {
            let pool = BufferPool::open(&name).unwrap();
            black_box(pool);
        });
    });
}

criterion_group!(
    benches,
    bench_pool_create,
    bench_buffer_acquire,
    bench_buffer_write_read,
    bench_ref_count_ops,
    bench_pool_open
);
criterion_main!(benches);
