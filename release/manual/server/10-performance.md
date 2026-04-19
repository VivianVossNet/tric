# Performance

TRIC+ ships with a built-in benchmark. Run it on your hardware to get numbers that matter for your deployment.

## Run the benchmark

Canonical runner (see `concept/driver/qa.md` §Benchmark for the full environment matrix):

```bash
cargo build --release
./target/release/tric server &
REDIS_URL="redis://127.0.0.1/" cargo test --release --test benchmark_test -- --ignored --nocapture
```

This measures three layers:

| Layer | What it measures | Transport |
|-------|-----------------|-----------|
| **1: In-process** | Raw engine speed. Library API, no transport overhead. | None |
| **2: Server (UDS)** | Server throughput via opcode `0x02 write_value` (with `duration_ms > 0` for transient writes) and `0x01 read_value`. One round-trip per operation, transient BTreeMap path. | Unix datagram socket |
| **3: Redis (TCP)** | Baseline comparison. Redis `SET`/`GET` via TCP localhost. | TCP |

Layer 2 requires a running `tric server` on `/var/run/tric/server.sock` (override via `TRIC_SOCKET_DIR`). Layer 3 requires a reachable Redis instance. Missing either layer silently skips those tests — verify numbers are present before publishing.

## Methodology

Every number in the benchmark follows these rules:

- **Synchronous roundtrip** — send request, wait for response, then next request
- **No pipelining** — one operation in flight at a time
- **No batching** — each operation is a separate datagram or command
- **128-byte values** — consistent payload size across all tests
- **Single client thread** — no concurrent clients (except the explicit concurrency test)
- **Same machine** — all three layers measured on identical hardware

This methodology is conservative. Pipelining and batching would increase throughput significantly, but single-operation latency is the honest metric.

## Reference numbers

### macOS (Apple Silicon)

| Layer | Workload | ops/s | p50 | p99 |
|-------|----------|------:|----:|----:|
| 1 | Transient write 128B | 2,492,116 | 292ns | 1.17µs |
| 1 | Transient read 128B | 4,198,248 | 208ns | 417ns |
| 1 | Cache-promoted read | 3,853,936 | 208ns | 458ns |
| 1 | SQLite write (WAL) | 18,696 | 43.2µs | 178.7µs |
| 1 | SQLite read (SQLite→cache) | 124,548 | 4.2µs | 51.5µs |
| 2 | Server write 128B (UDS, `0x02` + duration) | 21,112 | 42.7µs | 114.4µs |
| 2 | Server read 128B (UDS, `0x01`) | 30,255 | 31.8µs | 78.9µs |
| 3 | Redis write 128B (TCP) | 13,515 | 59.2µs | 251.3µs |
| 3 | Redis read 128B (TCP) | 38,476 | 23.7µs | 62.7µs |

### FreeBSD 15 (AMD Ryzen 5 3600, ZFS, Jail)

| Layer | Workload | ops/s | p50 | p99 |
|-------|----------|------:|----:|----:|
| 1 | Transient write 128B | 1,139,079 | 650ns | 3.87µs |
| 1 | Transient read 128B | 1,672,309 | 470ns | 2.12µs |
| 1 | Cache-promoted read | 2,318,271 | 340ns | 1.19µs |
| 1 | SQLite write (WAL/ZFS) | 18,177 | 24.6µs | 75.3µs |
| 1 | SQLite read (SQLite→cache) | 182,108 | 5.2µs | 9.9µs |
| 2 | Server write 128B (UDS, `0x02` + duration) | 19,065 | 34.6µs | 92.8µs |
| 2 | Server read 128B (UDS, `0x01`) | 130,379 | 7.4µs | 11.6µs |
| 3 | Redis write 128B (TCP) | 59,063 | 15.6µs | 40.8µs |
| 3 | Redis read 128B (TCP) | 82,633 | 11.4µs | 20.2µs |

## Interpreting the layers

**Layer 1 vs Layer 3** shows the architectural advantage of in-process storage over a network-based database. Use this when TRIC+ is embedded as a library: 60–200x faster than any network KV-store.

**Layer 2 vs Layer 3** is the server-to-server comparison. Both go through a transport layer (UDS DGRAM vs TCP). On FreeBSD, TRIC+ reads are **1.58x faster** than Redis reads (p50 7.4µs vs 11.4µs) — the UDS path + one-round-trip `0x02` write (with `duration_ms > 0`) beats a TCP handshake'd `SET EX`. Writes on FreeBSD favour Redis (Redis 59k ops/s vs TRIC+ 19k ops/s) — Redis' TCP write path is decades-tuned and single-threaded-synchronous writes are its home turf. On macOS the pattern flips: TRIC+ wins writes (1.56x), Redis wins reads (1.27x) — macOS UDS throughput is lower than FreeBSD's.

**Layer 1 vs Layer 2** shows the cost of the UDS transport layer itself. Roughly 30–100x slower than in-process — the difference is the kernel context switch per datagram plus parse/dispatch/encode on the server side.

## Tuning

### Transient tier

The transient tier (BTreeMap) is CPU-bound. Performance scales with single-core clock speed. The `Arc<RwLock<Store>>` serialises concurrent operations — under high contention, consider sharding by key prefix.

### Persistent tier

The persistent tier (SQLite) is I/O-bound. Performance scales with:

- **Filesystem:** ZFS with `sync=standard` is slower than ext4/UFS. Consider `sync=disabled` for non-critical data.
- **Storage medium:** NVMe > SSD > HDD
- **WAL mode:** Already enabled by default. Do not change to rollback journal.

### Cache-promotion window

The default cache-promotion TTL is 60 seconds. Frequently-read persistent data stays in BTreeMap for this window. The value is hardcoded at `CACHE_PROMOTION_SECONDS` in `permutive_bus.rs`. Adjust and rebuild if your workload benefits from a longer or shorter window.

## Redis comparison notes

Redis on FreeBSD (59–82k ops/s) performs significantly better than Redis on macOS (13–38k ops/s). This reflects macOS's higher TCP stack overhead, not Redis itself — FreeBSD's network stack is optimised for server workloads. macOS UDS also lags FreeBSD UDS, so TRIC+ Layer 2 shifts the same way.

The comparison is genuinely apples-to-apples only at Layer 2: TRIC+ uses opcode `0x02 write_value` with `duration_ms > 0`, which writes directly into the BTreeMap transient layer with TTL, exactly matching Redis' `SET k v EX t` semantics. Neither involves persistence; both cost one round-trip per operation.

TRIC+'s real architectural advantage is outside this benchmark: a permutive tier where keys with TTL live in BTreeMap and keys without TTL live in SQLite — one address space, one API, zero coordination. Redis needs a separate store for anything that must survive restart.

## Next

- [Installation](01-installation.md) — deploy on your platform
- [Storage Model](05-storage-model.md) — understand the routing
