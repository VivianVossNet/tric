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
- **Multi-shot median + CV gate** — every measurement runs 1 warm-up + 5 measurement shots; reported value is the median ops/s; published only if Coefficient of Variation ≤ 10 % (otherwise re-run, with explicit `MEASUREMENT UNRELIABLE` warning if the threshold is missed three times). Per-shot setup creates fresh state for Layer-1 library tests; persistent connection state for Layer-2/3 transport tests.

This methodology is conservative. Pipelining and batching would increase throughput significantly, but single-operation latency is the honest metric. The multi-shot reporting is governed by the source of truth in `concept/driver/qa.md` §Benchmark §Methodology.

## Reference numbers

### macOS (Apple Silicon)

| Layer | Workload | ops/s | p50 | p99 |
|-------|----------|------:|----:|----:|
| 1 | Transient write 128B | 2,492,116 | 292ns | 1.17µs |
| 1 | Transient read 128B | 4,198,248 | 208ns | 417ns |
| 1 | Cache-promoted read | 3,853,936 | 208ns | 458ns |
| 1 | SQLite write (WAL) | 18,696 | 43.2µs | 178.7µs |
| 1 | SQLite read (SQLite→cache) | 124,548 | 4.2µs | 51.5µs |
| 2 | Server write 128B (UDS, `0x02` + duration) | 16,437 | 54.0µs | 164.3µs |
| 2 | Server read 128B (UDS, `0x01`) | 26,962 | 34.6µs | 93.8µs |
| 3 | Redis write 128B (TCP) | 12,386 | 63.3µs | 268.7µs |
| 3 | Redis read 128B (TCP) | 37,972 | 23.8µs | 63.9µs |

### FreeBSD 15 (AMD Ryzen 5 3600, ZFS, Jail)

| Layer | Workload | ops/s | p50 | p99 |
|-------|----------|------:|----:|----:|
| 1 | Transient write 128B | 1,078,094 | 660ns | 4.53µs |
| 1 | Transient read 128B | 1,650,737 | 450ns | 3.12µs |
| 1 | Cache-promoted read | 2,241,860 | 360ns | 1.23µs |
| 1 | SQLite write (WAL/ZFS) | 17,477 | 25.3µs | 78.2µs |
| 1 | SQLite read (SQLite→cache) | 175,468 | 5.5µs | 10.3µs |
| 2 | Server write 128B (UDS, `0x02` + duration) | 39,920 | 23.7µs | 45.5µs |
| 2 | Server read 128B (UDS, `0x01`) | 105,590 | 8.6µs | 16.3µs |
| 3 | Redis write 128B (TCP) | 55,207 | 16.5µs | 42.9µs |
| 3 | Redis read 128B (TCP) | 96,276 | 10.2µs | 12.0µs |

## Interpreting the layers

**Layer 1 vs Layer 3** shows the architectural advantage of in-process storage over a network-based database. Use this when TRIC+ is embedded as a library: 60–200x faster than any network KV-store.

**Layer 2 vs Layer 3** is the server-to-server comparison. Both go through a transport layer (UDS DGRAM vs TCP). On FreeBSD, TRIC+ reads are **1.10x faster** than Redis reads (p50 8.6µs vs 10.2µs) — the UDS path + one-round-trip `0x02` write (with `duration_ms > 0`) just edges out a TCP handshake'd `SET EX`. Writes on FreeBSD favour Redis (Redis 55k ops/s vs TRIC+ 40k ops/s, 0.72x) — Redis' TCP write path is decades-tuned. On macOS the pattern flips: TRIC+ wins writes (1.33x), Redis wins reads (1.41x) — macOS UDS throughput is lower than FreeBSD's. Two of four quadrants favour TRIC+; pushing every quadrant past Redis is on the dedicated server-hot-path optimisation track.

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

Redis on FreeBSD (55–96k ops/s) performs significantly better than Redis on macOS (12–38k ops/s). This reflects macOS's higher TCP stack overhead, not Redis itself — FreeBSD's network stack is optimised for server workloads. macOS UDS also lags FreeBSD UDS, so TRIC+ Layer 2 shifts the same way.

The comparison is genuinely apples-to-apples only at Layer 2: TRIC+ uses opcode `0x02 write_value` with `duration_ms > 0`, which writes directly into the BTreeMap transient layer with TTL, exactly matching Redis' `SET k v EX t` semantics. Neither involves persistence; both cost one round-trip per operation.

TRIC+'s real architectural advantage is outside this benchmark: a permutive tier where keys with TTL live in BTreeMap and keys without TTL live in SQLite — one address space, one API, zero coordination. Redis needs a separate store for anything that must survive restart.

## Next

- [Installation](01-installation.md) — deploy on your platform
- [Storage Model](05-storage-model.md) — understand the routing
