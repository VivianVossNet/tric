// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Benchmark — built-in performance measurement for transient, persistent, and Redis comparison.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::data_bus::DataBus;
use crate::core::permutive_bus::create_permutive_bus;
use crate::create_tric;

pub fn run_benchmark() {
    println!("TRIC+ Performance Benchmark");
    println!("===========================");
    println!();
    println!("  Layer 1: In-process (library, no transport overhead)");
    println!("  Layer 2: Server (UDS DGRAM, kernel context switch per operation)");
    println!("  Layer 3: Redis (TCP localhost, full network stack per operation)");
    println!();

    println!("--- Layer 1: TRIC+ In-Process (library API, no transport) ---\n");
    check_transient_write();
    check_transient_read();
    check_transient_mixed();
    check_concurrent_write();
    check_persistent_write();
    check_persistent_read();
    check_cache_promoted_read();
    check_scan();

    println!("\n--- Layer 2: TRIC+ Server (UDS DGRAM, per-datagram roundtrip) ---\n");
    check_tric_server();

    println!("\n--- Layer 3: Redis (TCP localhost, per-command roundtrip) ---\n");
    check_redis();

    println!("\nMethodology: Each operation is a synchronous roundtrip (send + wait for");
    println!("response). No pipelining, no batching. 128-byte values. Single client thread.");
    println!("Layer 1 measures raw engine speed. Layers 2 and 3 measure real-world server");
    println!("throughput including transport overhead. All three layers on the same machine.");
}

fn create_key(index: usize) -> Vec<u8> {
    format!("bench:{index:08}").into_bytes()
}

fn create_value(size: usize) -> Vec<u8> {
    vec![0x42; size]
}

fn create_benchmark_bus() -> (Arc<dyn DataBus>, String) {
    let dir = format!("/tmp/tric-benchmark-run-{}", std::process::id());
    let _ = std::fs::remove_dir_all(&dir);
    let bus = Arc::new(create_permutive_bus(Path::new(&dir), "bench", 0));
    (bus, dir)
}

fn render_result(label: &str, operations: usize, duration: Duration, latencies: &mut [Duration]) {
    let ops_per_second = operations as f64 / duration.as_secs_f64();
    let avg = latencies.iter().map(|d| d.as_nanos()).sum::<u128>() / latencies.len() as u128;
    latencies.sort();
    let p50 = latencies[latencies.len() / 2].as_nanos();
    let p95 = latencies[latencies.len() * 95 / 100].as_nanos();
    let p99 = latencies[latencies.len() * 99 / 100].as_nanos();

    println!(
        "  {label:<40} {ops_per_second:>10.0} ops/s  avg {avg:>6}ns  p50 {p50:>6}ns  p95 {p95:>6}ns  p99 {p99:>6}ns"
    );
}

fn check_transient_write() {
    let tric = create_tric();
    let value = create_value(128);
    let count = 100_000;
    let mut latencies = Vec::with_capacity(count);

    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let key = create_key(index);
        tric.write_value(&key, &value);
        latencies.push(t.elapsed());
    }
    render_result(
        "transient write (128B)",
        count,
        start.elapsed(),
        &mut latencies,
    );
}

fn check_transient_read() {
    let tric = create_tric();
    let value = create_value(128);
    let count = 100_000;

    for index in 0..count {
        tric.write_value(&create_key(index), &value);
    }

    let mut latencies = Vec::with_capacity(count);
    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let _ = tric.read_value(&create_key(index));
        latencies.push(t.elapsed());
    }
    render_result(
        "transient read (128B)",
        count,
        start.elapsed(),
        &mut latencies,
    );
}

fn check_transient_mixed() {
    let tric = create_tric();
    let value = create_value(128);
    let count = 100_000;
    let mut latencies = Vec::with_capacity(count);

    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let key = create_key(index);
        if index % 2 == 0 {
            tric.write_value(&key, &value);
        } else {
            let _ = tric.read_value(&create_key(index.saturating_sub(1)));
        }
        latencies.push(t.elapsed());
    }
    render_result(
        "transient mixed 50/50 (128B)",
        count,
        start.elapsed(),
        &mut latencies,
    );
}

fn check_concurrent_write() {
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
    let total = thread_count * operations_per_thread;
    let ops = total as f64 / duration.as_secs_f64();
    println!(
        "  {:<40} {:>10.0} ops/s  ({thread_count} threads x {operations_per_thread} ops)",
        "concurrent write (4T, 128B)", ops
    );
}

fn check_persistent_write() {
    let (bus, dir) = create_benchmark_bus();
    let value = create_value(128);
    let count = 10_000;
    let mut latencies = Vec::with_capacity(count);

    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        bus.write_value(&create_key(index), &value);
        latencies.push(t.elapsed());
    }
    render_result(
        "persistent write (128B, SQLite WAL)",
        count,
        start.elapsed(),
        &mut latencies,
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn check_persistent_read() {
    let (bus, dir) = create_benchmark_bus();
    let value = create_value(128);
    let count = 10_000;

    for index in 0..count {
        bus.write_value(&create_key(index), &value);
    }

    let mut latencies = Vec::with_capacity(count);
    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let _ = bus.read_value(&create_key(index));
        latencies.push(t.elapsed());
    }
    render_result(
        "persistent read (128B, SQLite)",
        count,
        start.elapsed(),
        &mut latencies,
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn check_cache_promoted_read() {
    let (bus, dir) = create_benchmark_bus();
    let value = create_value(128);
    let count = 10_000;

    for index in 0..count {
        bus.write_value(&create_key(index), &value);
    }
    for index in 0..count {
        let _ = bus.read_value(&create_key(index));
    }

    let mut latencies = Vec::with_capacity(count);
    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let _ = bus.read_value(&create_key(index));
        latencies.push(t.elapsed());
    }
    render_result(
        "cache-promoted read (128B)",
        count,
        start.elapsed(),
        &mut latencies,
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn check_scan() {
    let tric = create_tric();
    let value = create_value(64);
    for index in 0..10_000 {
        tric.write_value(&format!("scan:{index:08}").into_bytes(), &value);
    }

    let count = 1_000;
    let mut latencies = Vec::with_capacity(count);
    let start = Instant::now();
    for _ in 0..count {
        let t = Instant::now();
        let _ = tric.find_by_prefix(b"scan:");
        latencies.push(t.elapsed());
    }
    render_result("scan (10k entries)", count, start.elapsed(), &mut latencies);
}

fn check_tric_server() {
    use std::os::unix::net::UnixDatagram;

    let socket_dir =
        std::env::var("TRIC_SOCKET_DIR").unwrap_or_else(|_| "/var/run/tric".to_string());
    let server_sock = format!("{socket_dir}/server.sock");
    let client_path = format!("/tmp/tric-bench-srv-{}.sock", std::process::id());
    let _ = std::fs::remove_file(&client_path);

    let client = match UnixDatagram::bind(&client_path) {
        Ok(client) => client,
        Err(_) => {
            println!("  Cannot bind client socket. Skipping server benchmark.");
            return;
        }
    };

    if client.connect(&server_sock).is_err() {
        let _ = std::fs::remove_file(&client_path);
        println!("  TRIC+ server is not running. To include server benchmark:");
        println!();
        println!("    Start the server first:  tric server &");
        println!("    Then re-run:             tric benchmark");
        return;
    }

    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let value = create_value(128);
    let count = 50_000;

    let mut latencies = Vec::with_capacity(count);
    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let key = create_key(index);
        let mut datagram = Vec::with_capacity(9 + key.len() + value.len());
        datagram.extend_from_slice(&(index as u32).to_be_bytes());
        datagram.push(0x02);
        datagram.extend_from_slice(&(key.len() as u32).to_be_bytes());
        datagram.extend_from_slice(&key);
        datagram.extend_from_slice(&(value.len() as u32).to_be_bytes());
        datagram.extend_from_slice(&value);
        let _ = client.send(&datagram);
        let mut buffer = [0u8; 64];
        let _ = client.recv(&mut buffer);
        latencies.push(t.elapsed());
    }
    render_result(
        "server write (128B, UDS DGRAM)",
        count,
        start.elapsed(),
        &mut latencies,
    );

    let mut latencies = Vec::with_capacity(count);
    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let key = create_key(index);
        let mut datagram = Vec::with_capacity(9 + key.len());
        datagram.extend_from_slice(&(index as u32).to_be_bytes());
        datagram.push(0x01);
        datagram.extend_from_slice(&(key.len() as u32).to_be_bytes());
        datagram.extend_from_slice(&key);
        let _ = client.send(&datagram);
        let mut buffer = [0u8; 256];
        let _ = client.recv(&mut buffer);
        latencies.push(t.elapsed());
    }
    render_result(
        "server read (128B, UDS DGRAM)",
        count,
        start.elapsed(),
        &mut latencies,
    );

    let _ = std::fs::remove_file(&client_path);
}

fn check_redis() {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());

    let connection = match try_redis_connection(&url) {
        Some(connection) => connection,
        None => {
            println!("  Redis is not running. To include Redis comparison:");
            println!();
            println!("    macOS:    brew install redis && redis-server --daemonize yes");
            println!("    FreeBSD:  pkg install redis && redis-server --daemonize yes --bind 127.0.0.1 --protected-mode no");
            println!("    Linux:    apt install redis-server && redis-server --daemonize yes");
            println!();
            println!("  Then re-run:  tric benchmark");
            println!("  With auth:    REDIS_URL=\"redis://:password@host/\" tric benchmark");
            return;
        }
    };

    check_redis_write(connection);

    let connection = try_redis_connection(&url).unwrap();
    check_redis_read(connection);
}

fn try_redis_connection(url: &str) -> Option<redis::Connection> {
    let client = redis::Client::open(url).ok()?;
    client.get_connection().ok()
}

fn check_redis_write(mut connection: redis::Connection) {
    let value = create_value(128);
    let count = 100_000;
    let mut latencies = Vec::with_capacity(count);

    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let key = create_key(index);
        let _: Result<(), _> = redis::cmd("SET")
            .arg(&key)
            .arg(&value)
            .query(&mut connection);
        latencies.push(t.elapsed());
    }
    render_result(
        "redis write (128B, TCP localhost)",
        count,
        start.elapsed(),
        &mut latencies,
    );
}

fn check_redis_read(mut connection: redis::Connection) {
    let count = 100_000;
    let mut latencies = Vec::with_capacity(count);

    let start = Instant::now();
    for index in 0..count {
        let t = Instant::now();
        let key = create_key(index);
        let _: Result<Vec<u8>, _> = redis::cmd("GET").arg(&key).query(&mut connection);
        latencies.push(t.elapsed());
    }
    render_result(
        "redis read (128B, TCP localhost)",
        count,
        start.elapsed(),
        &mut latencies,
    );
}
