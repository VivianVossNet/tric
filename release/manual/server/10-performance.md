# Performance

TRIC+ ships with a built-in benchmark. Run it on your hardware to get numbers that matter for your deployment.

## Run the benchmark

```bash
tric -b
```

This measures three layers:

| Layer | What it measures | Transport |
|-------|-----------------|-----------|
| **1: In-process** | Raw engine speed. Library API, no transport overhead. | None |
| **2: Server (UDS)** | Real-world server throughput. UDS DGRAM roundtrip per operation. | Unix datagram socket |
| **3: Redis (TCP)** | Baseline comparison. Redis SET/GET via TCP localhost. | TCP |

Layer 2 requires a running TRIC+ server (`tric server &`). Layer 3 requires a running Redis instance.

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
| 1 | Transient write | 2,900,000 | 292ns | 1.5us |
| 1 | Transient read | 4,700,000 | 167ns | 292ns |
| 1 | Cache-promoted read | 4,700,000 | 167ns | 292ns |
| 1 | SQLite write (WAL) | 60,000 | 13us | 39us |
| 1 | SQLite read | 350,000 | 2.7us | 5.9us |
| 2 | Server write (UDS) | 21,000 | 43us | 88us |
| 2 | Server read (UDS) | 38,000 | 24us | 62us |
| 3 | Redis write (TCP) | 8,700 | 108us | 183us |
| 3 | Redis read (TCP) | 8,700 | 109us | 181us |

### FreeBSD 15 (AMD Ryzen 5 3600, ZFS, Jail)

| Layer | Workload | ops/s | p50 | p99 |
|-------|----------|------:|----:|----:|
| 1 | Transient write | 1,600,000 | 500ns | 2.7us |
| 1 | Transient read | 1,800,000 | 440ns | 1.5us |
| 1 | Cache-promoted read | 2,400,000 | 360ns | 1.0us |
| 1 | SQLite write (WAL/ZFS) | 15,000 | 21us | 62us |
| 1 | SQLite read | 220,000 | 4.1us | 7.0us |
| 3 | Redis write (TCP) | 73,000 | 13us | 32us |
| 3 | Redis read (TCP) | 95,000 | 10us | 13us |

## Interpreting the layers

**Layer 1 vs Layer 3** shows the architectural advantage of in-process storage over a network-based database. This is the number to use when TRIC+ is embedded as a library.

**Layer 2 vs Layer 3** is the fair server-to-server comparison. Both go through a transport layer (UDS vs TCP). TRIC+ over UDS is 2–4x faster than Redis over TCP on the same machine.

**Layer 1 vs Layer 2** shows the cost of the UDS transport layer itself. The difference is the kernel context switch per datagram.

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

Redis on FreeBSD (73–95k ops/s) performs significantly better than Redis on macOS (5–9k ops/s). This is due to macOS's higher TCP stack overhead, not Redis itself. FreeBSD's network stack is optimised for server workloads.

TRIC+ performance is more consistent across platforms because the transient tier avoids the network stack entirely, and the persistent tier uses SQLite's cross-platform WAL implementation.

## Next

- [Installation](01-installation.md) — deploy on your platform
- [Storage Model](05-storage-model.md) — understand the routing
