# TRIC+

**The database that decides where your data lives, so you do not have to.**

Every production stack that needs both a cache and a database asks the developer to choose, configure two systems, and keep them in sync by hand. TRIC+ does not ask. Write a value with a time-to-live and it lives in nanosecond-fast memory. Write it without one and it lives on disk. The API does not change. You think in how long your data needs to live; the engine decides where it lives.

This is **permutive storage**: data permutes between a transient tier (a `BTreeMap` in memory) and a persistent tier (SQLite on disk) according to lifetime, without a single configuration knob. One engine. One wire protocol. Six primitives that cover what production actually needs: read, write, delete, compare-and-delete, time-to-live, prefix scan.

You could assemble this yourself: a `BTreeMap`, a `rusqlite` binding, a lazy TTL sweeper, a cache-promotion pass, a wire protocol with per-datagram encryption and traffic-shape padding, atomic compare-and-delete on the server, prefix-scan as a first-class operation, a reproducible benchmark harness. TRIC+ is what happens when someone already did.

On FreeBSD, the canonical deployment target, TRIC+ is faster than Redis in both read and write quadrants of the server-to-server comparison. Waves 1 and 2 of the bridge programme are complete; Wave 3 covers enterprise languages (PHP, Java, Kotlin, Python, Ruby, C#, Go), and Wave 4 covers native-socket clients for the remaining ecosystems. The server is free for any single-host production use under the Business Source License, and every tagged version converts to Apache 2.0 four years after release. The reader who needed a KV store that was honest about persistence, fast on the platform they actually run, and legally clear now has one.

![C](https://img.shields.io/badge/C-ready-4caf50?style=flat-square&logo=c)
![C++](https://img.shields.io/badge/C++-ready-4caf50?style=flat-square&logo=cplusplus)
![Swift](https://img.shields.io/badge/Swift-ready-4caf50?style=flat-square&logo=swift)
![Nim](https://img.shields.io/badge/Nim-ready-4caf50?style=flat-square&logo=nim)
![Lua](https://img.shields.io/badge/Lua-ready-4caf50?style=flat-square&logo=lua)
![Tcl](https://img.shields.io/badge/Tcl-ready-4caf50?style=flat-square)
![Zig](https://img.shields.io/badge/Zig-ready-4caf50?style=flat-square&logo=zig)
![PHP](https://img.shields.io/badge/PHP-planned-555555?style=flat-square&logo=php)
![Java](https://img.shields.io/badge/Java-planned-555555?style=flat-square&logo=openjdk)
![Kotlin](https://img.shields.io/badge/Kotlin-planned-555555?style=flat-square&logo=kotlin)
![Python](https://img.shields.io/badge/Python-planned-555555?style=flat-square&logo=python)
![Ruby](https://img.shields.io/badge/Ruby-planned-555555?style=flat-square&logo=ruby)
![C#](https://img.shields.io/badge/C%23-planned-555555?style=flat-square&logo=dotnet)
![Go](https://img.shields.io/badge/Go-planned-555555?style=flat-square&logo=go)
![JavaScript](https://img.shields.io/badge/JavaScript-planned-555555?style=flat-square&logo=javascript)
![TypeScript](https://img.shields.io/badge/TypeScript-planned-555555?style=flat-square&logo=typescript)
![Perl](https://img.shields.io/badge/Perl-planned-555555?style=flat-square&logo=perl)
![Elixir](https://img.shields.io/badge/Elixir-planned-555555?style=flat-square&logo=elixir)
![Dart](https://img.shields.io/badge/Dart-planned-555555?style=flat-square&logo=dart)
![Rust](https://img.shields.io/badge/Rust-native-dea584?style=flat-square&logo=rust)

Write a value. Set a TTL and it lives in a `BTreeMap`. Don't set a TTL and it lives in SQLite: one database file per namespace, WAL mode for concurrent reads, cache-promotion of hot keys back into memory on read. Not SQLite bolted on next to a cache; one engine, two tiers, same six primitives. The developer thinks in lifetimes, not systems.

## What it does

- **Six primitives:** read, write, delete, compare-and-delete, TTL, prefix scan. Nothing else; every higher-level pattern composes from these.
- **Two storage tiers:** transient (`BTreeMap`, nanosecond access) and persistent (SQLite, survives restarts). Transparent to the caller.
- **Automatic routing:** TTL present places the value in the transient tier. No TTL places it in persistent storage. Reads promote hot persistent keys into the transient cache for one minute.
- **Single binary:** `tric server` starts the daemon, `tric <command>` is the CLI. No external runtime dependency.
- **Wire protocol:** UDS DGRAM for local clients, UDP for network clients. Each network datagram is encrypted with ChaCha20-Poly1305 and padded with random noise, so an observer on the wire cannot determine operation type or payload size.
- **SQL interface:** `tric query "SELECT * FROM users WHERE key = '42'"`. The SQL layer reads directly from the persistent tier.
- **Import / Export:** native SQL dumps (MySQL, PostgreSQL, SQLite), `.tric` Brotli-compressed archives, differential imports that apply only what changed.
- **Instance management:** multiple projects live under `/var/db/tric/`, each with slot-based clones for staging, migration, and A/B work.
- **20 language bridges** planned, 7 ready today (C, C++, Swift, Nim, Lua, Tcl, Zig; Waves 1 and 2 complete). C is the FFI foundation; later waves use native sockets for each language's idiomatic client.
- **8 CMS and shop integrations** planned on top of the Wave 3 bridges: WordPress, Drupal, Craft CMS, WooCommerce, Magento, Shopify, PrestaShop, Umbraco.

## Performance

Three benchmark layers, one machine, one payload, one methodology. Measured on **FreeBSD 15** (AMD Ryzen 5 3600, ZFS, Jail), which is the canonical deployment target for TRIC+. Layer 2 is the apples-to-apples comparison: TRIC+ over UDS against Redis over TCP, both serving an in-memory `SET k v EX t` / `GET k` workload.

| Layer | Workload | ops/s | p50 | p99 | vs Redis |
|-------|----------|------:|----:|----:|---------:|
| 1: In-process | Transient write 128 B | 1,645,225 | 500 ns | 1.47 µs | n/a |
| 1: In-process | Transient read 128 B | 2,199,671 | 370 ns | 770 ns | n/a |
| 1: In-process | Cache-promoted read | 2,737,603 | 320 ns | 570 ns | n/a |
| 1: In-process | SQLite write (WAL, ZFS) | 18,114 | 23.9 µs | 68.1 µs | n/a |
| **2: Server (UDS)** | **Write 128 B** | **67,570** | **14.5 µs** | **22.9 µs** | **1.03x** |
| **2: Server (UDS)** | **Read 128 B** | **91,675** | **10.6 µs** | **17.5 µs** | **1.16x** |
| 3: Redis (TCP) | Write 128 B | 65,383 | 15.2 µs | 16.8 µs | baseline |
| 3: Redis (TCP) | Read 128 B | 79,129 | 12.3 µs | 22.1 µs | baseline |

**Layer 1** measures raw engine speed in-process, no transport. **Layer 2** measures server throughput via UDS DGRAM using opcode `0x02 write_value` with `duration_ms > 0`: one round-trip on the transient `BTreeMap` path, directly comparable to Redis' `SET k v EX t`. **Layer 3** measures Redis via TCP localhost. All benchmarks run single-threaded and synchronously, without pipelining or batching.

The single-shot SET and GET above are Redis' home discipline, and TRIC+ beats Redis on both quadrants on FreeBSD. TRIC+'s own architectural strengths, permutive routing, cache-promotion, prefix-scan as a first-class operation, atomic compare-and-delete without scripting, and concurrent multi-client mixed workloads, are characterised in [`release/manual/server/10-performance.md`](release/manual/server/10-performance.md) under §TRIC+-specific workloads. Among them: atomic CAS is 292 times faster than the Redis Lua-scripting equivalent, and prefix scan is 65 times faster than `KEYS *`.

### Reproduce

```bash
# Build the release binary first. cargo test does not rebuild it.
cargo build --release

# Start a TRIC+ server in the background.
./target/release/tric server &

# Run the full benchmark matrix. Redis must be reachable.
REDIS_URL="redis://127.0.0.1/" cargo test --release --test benchmark_test -- --ignored --nocapture
```

## Installation

### As a library

```toml
[dependencies]
tric = "0.5.YYMMDDHHMM"  # see latest release
```

### As a server

```bash
cargo build --release
TRIC_BASE_DIR=/var/db/tric TRIC_INSTANCE=myapp TRIC_SLOT=0 ./target/release/tric server
```

## Library API

The six primitives are the whole surface. Everything else composes from them.

```rust
use tric::{create_tric, Bytes};
use std::time::Duration;

let tric = create_tric();

// Persistent: no TTL, so the value lives in SQLite when used via the server.
tric.write_value(b"user:42", b"alice");
assert_eq!(tric.read_value(b"user:42"), Some(Bytes::from_static(b"alice")));

// Transient: a TTL routes the value into the BTreeMap tier, gone after expiry.
tric.write_value(b"session:abc", b"token");
tric.write_ttl(b"session:abc", Duration::from_secs(3600));

// Prefix scan, sorted, first-class.
let users = tric.find_by_prefix(b"user:");

// Compare-and-delete: atomic job claiming with no scripting.
tric.delete_value_if_match(b"job:1", b"pending");
```

### Primitives

| Method | Purpose |
|--------|---------|
| `read_value(key)` | Return the value, or `None` if absent or expired |
| `write_value(key, value)` | Store the value and clear any existing TTL |
| `delete_value(key)` | Remove the key and its TTL state. Silent on missing keys |
| `delete_value_if_match(key, expected)` | Delete only if the current value equals `expected` |
| `write_ttl(key, duration)` | Attach an expiry to an existing key. Silent on missing keys |
| `find_by_prefix(prefix)` | Return all matching `(key, value)` pairs, sorted |

## Server CLI

FreeBSD-style short flags, no GNU long options.

```
usage: tric <command> [args...]
       tric server                          start daemon
       tric status                          server status
       tric keys [-p prefix]               list keys
       tric inspect <key>                   key metadata
       tric query <SQL>                     SQL query
       tric import -f <path> -F <dialect> [-a]
       tric import -D <old.tric> <new.tric> diff-import
       tric export -f <path> [-d] [-F <dialect>]
       tric dump -f <path>                  binary store dump
       tric restore -f <path>               binary store restore
       tric slots                           list instance slots
       tric clone <slot>                    clone current slot
       tric -b                              performance benchmark
       tric shutdown                        stop server
       tric shell                           interactive REPL
       tric -h                              help
```

| Flag | Purpose |
|------|---------|
| `-f <path>` | File path |
| `-F <dialect>` | SQL dialect (`mysql`, `postgres`, `sqlite`) |
| `-p <prefix>` | Key prefix filter |
| `-D <old> <new>` | Diff-import between two `.tric` snapshots |
| `-d` | Debug mode (uncompressed export) |
| `-a` | Analyse only (dry-run import) |
| `-b` | Run performance benchmark |
| `-h` | Help |

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `TRIC_SOCKET_DIR` | `/var/run/tric` | UDS socket directory |
| `TRIC_UDP_BIND` | `0.0.0.0:7483` | UDP listen address |
| `TRIC_BASE_DIR` | `/var/db/tric` | SQLite storage base directory |
| `TRIC_INSTANCE` | `default` | Instance name |
| `TRIC_SLOT` | `0` | Active slot (`_0` = primary, `_1`+ = clones) |
| `REDIS_URL` | `redis://127.0.0.1/` | Redis URL for benchmark comparison |

### Instance layout

```
/var/db/tric/
├── myapp_0/          # primary
│   ├── users.db
│   ├── orders.db
│   └── _schema.db
├── myapp_1/          # clone (staging, migration test)
│   └── ...
└── analytics_0/      # separate instance
    └── ...
```

## Wire protocol

Binary protocol over UDS DGRAM (local) and UDP (network). Each network datagram is encrypted with ChaCha20-Poly1305 and padded with random noise, so that an observer on the wire cannot determine operation type or payload size.

| Opcode range | Family |
|--------------|--------|
| `0x01`–`0x07` | Core primitives (read, write, delete, CAS, TTL, scan, query) |
| `0x10`–`0x1A` | Control / Admin (auth, ping, status, shutdown, reload, keys, inspect, dump, restore) |
| `0x80`–`0x81` | Success responses |
| `0x90`–`0x91` | Streaming (scan chunks) |
| `0xA0`–`0xA7` | Error responses |
| `0xB0` | Auth handshake |

## Language bridges

20 languages in four waves. Each bridge undergoes the same quality gate as the core engine: a dedicated test suite against a running server, strict compilation with warnings as errors, and the project's Hafenrundfahrt before every merge.

| Wave | Languages | Mechanism | Ready |
|------|-----------|-----------|-------|
| 1 | **C** | Shared library (`.so` / `.dylib`), FFI base | 1/1 |
| 2 | **C++**, **Swift**, **Nim**, **Lua**, **Tcl**, **Zig** | C FFI consumers | 6/6 |
| 3 | PHP, Java, Kotlin, Python, Ruby, C#/.NET, Go | Native socket | 0/7 |
| 4 | JavaScript, TypeScript, Perl, Elixir, Dart, Rust | Native socket | 0/6 |

**Bold** means production-ready. Every ready bridge ships with a quickstart manual under [`release/manual/clients/`](release/manual/clients/) and an integration test suite.

### CMS and shop integrations (planned)

WordPress, Drupal, Craft CMS, WooCommerce, Magento, Shopify, PrestaShop, Umbraco. Each builds on the corresponding Wave 3 bridge.

## Design

**`BTreeMap`, not `HashMap`.** Range queries walk contiguous entries, TTL management iterates from the oldest expiry, and neither works on a `HashMap`.

**Lazy expiry.** No background thread watches for timeouts. Every operation purges expired entries first; the cost is paid by the next caller, not by a scheduler thread that runs whether or not the store is busy.

**Permutive routing.** `write_value` without a TTL goes to SQLite. An additional `write_ttl` call moves the data into the `BTreeMap`. Reading from SQLite promotes the key into the `BTreeMap` cache for 60 seconds. The boundary between tiers is invisible to the caller; they see one API.

**Scoped SQLite.** Each namespace (the key prefix before `:`) gets its own `.db` file. No cross-table locks, parallel I/O, WAL mode with `NORMAL` synchronous.

**FreeBSD-first CLI.** Single-letter flags, terse output, `usage:` format, no GNU long options. Defaults follow FreeBSD filesystem conventions (`/var/db/`, `/var/run/`).

## Size

Roughly 3,500 lines of Rust across 20 source files. Single binary approximately 5 MB (SQLite bundled). 34 unit and integration tests including a server-roundtrip with persistence. 9 benchmark scenarios characterising both the Redis-comparison and TRIC+-specific workloads.

## Licence

Dual model. Use whichever fits.

**Server (`tric` crate, binary and library)** is licensed under the **Business Source License 1.1**. Non-production use is unrestricted. Production use on a single host is granted free of charge under the Additional Use Grant; a "host" is one OS instance (one bare-metal machine, VM, container, or FreeBSD jail each count as one). Production across more than one host, or offering TRIC+ as a managed service to third parties, requires a commercial licence. The server converts to Apache-2.0 on **2030-04-19**, or four years after the release date of each tagged version, whichever comes first. See [`LICENSE`](LICENSE) for the full text and [`LICENSE-APACHE`](LICENSE-APACHE) for the Change License.

**Language bridges (`bridges/`)** are licensed under the **BSD 3-Clause License**. No production restriction, no Change Date. Use them with any TRIC+ server, any application, any deployment model. See [`bridges/LICENSE`](bridges/LICENSE).

Copyright (c) 2025-2026 Vivian Voss. For commercial licensing, contact <https://vivianvoss.net/tric>.
