// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Performance benchmark — measures throughput and latency for transient, persistent, and mixed workloads.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tric::core::data_bus::DataBus;
use tric::core::permutive_bus::create_permutive_bus;
use tric::create_tric;

fn create_benchmark_dir(label: &str) -> String {
    let dir = format!("/tmp/tric-benchmark-{}-{}", label, std::process::id());
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn create_benchmark_bus(label: &str) -> (Arc<dyn DataBus>, String) {
    let dir = create_benchmark_dir(label);
    let bus = Arc::new(create_permutive_bus(Path::new(&dir), "bench", 0));
    (bus, dir)
}

fn create_key(index: usize) -> Vec<u8> {
    format!("bench:{index:08}").into_bytes()
}

fn create_value(size: usize) -> Vec<u8> {
    vec![0x42; size]
}

struct BenchmarkResult {
    label: &'static str,
    operations: usize,
    duration: Duration,
    latencies: Vec<Duration>,
}

impl BenchmarkResult {
    fn render(&self) -> String {
        let ops_per_second = self.operations as f64 / self.duration.as_secs_f64();
        let avg_microseconds = self.latencies.iter().map(|d| d.as_nanos()).sum::<u128>()
            / self.latencies.len() as u128;

        let mut sorted = self.latencies.clone();
        sorted.sort();
        let p50 = sorted[sorted.len() / 2].as_nanos();
        let p95 = sorted[sorted.len() * 95 / 100].as_nanos();
        let p99 = sorted[sorted.len() * 99 / 100].as_nanos();

        format!(
            "  {:<40} {:>10.0} ops/s  avg {:>6}ns  p50 {:>6}ns  p95 {:>6}ns  p99 {:>6}ns",
            self.label, ops_per_second, avg_microseconds, p50, p95, p99
        )
    }
}

fn run_benchmark<F>(label: &'static str, operations: usize, mut operation: F) -> BenchmarkResult
where
    F: FnMut(usize),
{
    let mut latencies = Vec::with_capacity(operations);

    let total_start = Instant::now();
    for index in 0..operations {
        let start = Instant::now();
        operation(index);
        latencies.push(start.elapsed());
    }
    let duration = total_start.elapsed();

    BenchmarkResult {
        label,
        operations,
        duration,
        latencies,
    }
}

#[test]
#[ignore]
fn check_benchmark_transient_write() {
    let tric = create_tric();
    let value = create_value(128);
    let count = 100_000;

    let result = run_benchmark("transient write (128B value)", count, |index| {
        let key = create_key(index);
        tric.write_value(&key, &value);
    });

    eprintln!("{}", result.render());
    assert!(
        result.operations as f64 / result.duration.as_secs_f64() > 100_000.0,
        "transient write should exceed 100k ops/s"
    );
}

#[test]
#[ignore]
fn check_benchmark_transient_read() {
    let tric = create_tric();
    let value = create_value(128);
    let count = 100_000;

    for index in 0..count {
        let key = create_key(index);
        tric.write_value(&key, &value);
    }

    let result = run_benchmark("transient read (128B value)", count, |index| {
        let key = create_key(index);
        let _ = tric.read_value(&key);
    });

    eprintln!("{}", result.render());
    assert!(
        result.operations as f64 / result.duration.as_secs_f64() > 100_000.0,
        "transient read should exceed 100k ops/s"
    );
}

#[test]
#[ignore]
fn check_benchmark_transient_mixed() {
    let tric = create_tric();
    let value = create_value(128);
    let count = 100_000;

    let result = run_benchmark("transient mixed 50/50 rw (128B)", count, |index| {
        let key = create_key(index);
        if index % 2 == 0 {
            tric.write_value(&key, &value);
        } else {
            let read_key = create_key(index.saturating_sub(1));
            let _ = tric.read_value(&read_key);
        }
    });

    eprintln!("{}", result.render());
    assert!(
        result.operations as f64 / result.duration.as_secs_f64() > 80_000.0,
        "transient mixed should exceed 80k ops/s"
    );
}

#[test]
#[ignore]
fn check_benchmark_persistent_write() {
    let (bus, dir) = create_benchmark_bus("pw");
    let value = create_value(128);
    let count = 10_000;

    let result = run_benchmark("persistent write (128B, SQLite)", count, |index| {
        let key = create_key(index);
        bus.write_value(&key, &value);
    });

    let ops = result.operations as f64 / result.duration.as_secs_f64();
    eprintln!("{}", result.render());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        ops > 500.0,
        "persistent write should exceed 500 ops/s (ZFS/WAL): got {ops:.0}"
    );
}

#[test]
#[ignore]
fn check_benchmark_persistent_read() {
    let (bus, dir) = create_benchmark_bus("pr");
    let value = create_value(128);
    let count = 10_000;

    for index in 0..count {
        let key = create_key(index);
        bus.write_value(&key, &value);
    }

    let result = run_benchmark("persistent read (128B, SQLite→cache)", count, |index| {
        let key = create_key(index);
        let _ = bus.read_value(&key);
    });

    eprintln!("{}", result.render());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        result.operations as f64 / result.duration.as_secs_f64() > 5_000.0,
        "persistent read should exceed 5k ops/s"
    );
}

#[test]
#[ignore]
fn check_benchmark_persistent_read_cached() {
    let (bus, dir) = create_benchmark_bus("prc");
    let value = create_value(128);
    let count = 10_000;

    for index in 0..count {
        let key = create_key(index);
        bus.write_value(&key, &value);
    }

    for index in 0..count {
        let key = create_key(index);
        let _ = bus.read_value(&key);
    }

    let result = run_benchmark("persistent read (cache-promoted)", count, |index| {
        let key = create_key(index);
        let _ = bus.read_value(&key);
    });

    eprintln!("{}", result.render());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        result.operations as f64 / result.duration.as_secs_f64() > 50_000.0,
        "cache-promoted read should exceed 50k ops/s"
    );
}

#[test]
#[ignore]
fn check_benchmark_scan() {
    let tric = create_tric();
    let value = create_value(64);
    let count = 10_000;

    for index in 0..count {
        let key = format!("scan:{index:08}").into_bytes();
        tric.write_value(&key, &value);
    }

    let result = run_benchmark("transient scan (10k entries)", 1_000, |_| {
        let _ = tric.find_by_prefix(b"scan:");
    });

    eprintln!("{}", result.render());
    assert!(
        result.operations as f64 / result.duration.as_secs_f64() > 100.0,
        "full scan of 10k entries should exceed 100 scans/s"
    );
}

#[test]
#[ignore]
fn check_benchmark_concurrent_write() {
    let tric = create_tric();
    let thread_count = 4;
    let operations_per_thread = 25_000;
    let value = create_value(128);

    let start = Instant::now();
    let mut handles = Vec::new();

    for thread_id in 0..thread_count {
        let tric = tric.clone();
        let value = value.clone();
        handles.push(std::thread::spawn(move || {
            for index in 0..operations_per_thread {
                let key = format!("conc:{thread_id}:{index:08}").into_bytes();
                tric.write_value(&key, &value);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let total_ops = thread_count * operations_per_thread;
    let ops_per_second = total_ops as f64 / duration.as_secs_f64();

    eprintln!(
        "  {:<40} {:>10.0} ops/s  ({} threads × {} ops in {:?})",
        "concurrent write (4 threads, 128B)",
        ops_per_second,
        thread_count,
        operations_per_thread,
        duration
    );

    assert!(
        ops_per_second > 50_000.0,
        "concurrent write should exceed 50k ops/s total"
    );
}

#[test]
#[ignore]
fn check_benchmark_value_sizes() {
    let tric = create_tric();
    let count = 10_000;

    for size in [32, 128, 512, 2048, 8192] {
        let value = create_value(size);
        let label = format!("transient write ({size}B value)");
        let label_static: &'static str = Box::leak(label.into_boxed_str());

        let result = run_benchmark(label_static, count, |index| {
            let key = create_key(index);
            tric.write_value(&key, &value);
        });

        eprintln!("{}", result.render());
    }
}
