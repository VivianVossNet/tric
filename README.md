# TRIC+

**Permutive Database Engine** — data permutes between transient memory and persistent storage based on lifetime, not configuration.

![C](https://img.shields.io/badge/C-ready-4caf50?style=flat-square&logo=c)
![C++](https://img.shields.io/badge/C++-ready-4caf50?style=flat-square&logo=cplusplus)
![Swift](https://img.shields.io/badge/Swift-ready-4caf50?style=flat-square&logo=swift)
![Nim](https://img.shields.io/badge/Nim-planned-555555?style=flat-square&logo=nim)
![Lua](https://img.shields.io/badge/Lua-planned-555555?style=flat-square&logo=lua)
![Tcl](https://img.shields.io/badge/Tcl-planned-555555?style=flat-square)
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

Write a value. Set a TTL and it lives in a BTreeMap. Don't set a TTL and it lives in SQLite. The API doesn't change. The developer thinks in lifetimes, not systems.

## What it does

- **Six primitives:** read, write, delete, compare-and-delete, TTL, prefix scan
- **Two storage tiers:** transient (BTreeMap, nanosecond access) and persistent (SQLite, survives restarts)
- **Automatic routing:** TTL present = transient. No TTL = persistent. Cache-promotion on read.
- **Single binary:** `tric server` starts the daemon, `tric <command>` is the CLI
- **Wire protocol:** UDS DGRAM (local) + UDP (network) with per-datagram ChaCha20-Poly1305 encryption
- **SQL interface:** `tric query "SELECT * FROM users WHERE key = '42'"`
- **Import/Export:** SQL dumps (MySQL, PostgreSQL, SQLite), `.tric` Brotli-compressed archives, diff-import
- **Instance management:** multiple projects with slot-based cloning under `/var/db/tric/`
- **20 language bridges** planned, 4 ready (C, C++, Swift, Zig) — C FFI base, then native socket clients for each language
- **8 CMS/shop integrations** planned (WordPress, Drupal, Craft CMS, WooCommerce, Magento, Shopify, PrestaShop, Umbraco)

## Performance

Three benchmark layers, same machine, same payload, same methodology. Layer 2 (server vs server) is the apples-to-apples comparison: TRIC+ over UDS against Redis over TCP, both serving an in-memory `SET k v EX t` / `GET k` workload.

### macOS (Apple Silicon)

| Layer | Workload | ops/s | p50 | p99 | vs Redis |
|-------|----------|------:|----:|----:|---------:|
| 1: In-process | Transient write 128B | 2,492,116 | 292ns | 1.17µs | — |
| 1: In-process | Transient read 128B | 4,198,248 | 208ns | 417ns | — |
| 1: In-process | Cache-promoted read | 3,853,936 | 208ns | 458ns | — |
| 1: In-process | SQLite write (WAL) | 18,696 | 43.2µs | 178.7µs | — |
| **2: Server (UDS)** | **Write 128B** | **16,437** | **54.0µs** | **164.3µs** | **1.33x** |
| **2: Server (UDS)** | **Read 128B** | **26,962** | **34.6µs** | **93.8µs** | 0.71x |
| 3: Redis (TCP) | Write 128B | 12,386 | 63.3µs | 268.7µs | baseline |
| 3: Redis (TCP) | Read 128B | 37,972 | 23.8µs | 63.9µs | baseline |

### FreeBSD 15 (AMD Ryzen 5 3600, ZFS, Jail)

| Layer | Workload | ops/s | p50 | p99 | vs Redis |
|-------|----------|------:|----:|----:|---------:|
| 1: In-process | Transient write 128B | 1,645,225 | 500ns | 1.47µs | — |
| 1: In-process | Transient read 128B | 2,199,671 | 370ns | 770ns | — |
| 1: In-process | Cache-promoted read | 2,737,603 | 320ns | 570ns | — |
| 1: In-process | SQLite write (WAL/ZFS) | 18,114 | 23.9µs | 68.1µs | — |
| **2: Server (UDS)** | **Write 128B** | **67,570** | **14.5µs** | **22.9µs** | **1.03x** |
| **2: Server (UDS)** | **Read 128B** | **91,675** | **10.6µs** | **17.5µs** | **1.16x** |
| 3: Redis (TCP) | Write 128B | 65,383 | 15.2µs | 16.8µs | baseline |
| 3: Redis (TCP) | Read 128B | 79,129 | 12.3µs | 22.1µs | baseline |

**Layer 1** measures raw engine speed (in-process, no transport). **Layer 2** measures server throughput via UDS DGRAM using opcode `0x02 write_value` with `duration_ms > 0` — one round-trip, transient BTreeMap path, directly comparable to Redis' `SET k v EX t`. **Layer 3** measures Redis via TCP localhost. All single-threaded, synchronous, no pipelining, no batching.

The single-shot SET/GET above is Redis' home discipline. TRIC+'s own architectural strengths — permutive routing (TTL = transient, no TTL = persistent, one API), cache-promotion, prefix-scan as a first-class operation, atomic CAS without scripting, concurrent multi-client mixed workloads — are characterised in [`release/manual/server/10-performance.md`](release/manual/server/10-performance.md) §TRIC+-specific workloads.

TRIC+ beats Redis on FreeBSD in both Layer-2 quadrants: writes (1.03x) and reads (1.16x). On macOS, TRIC+ leads writes; Redis leads reads due to macOS's stronger TCP stack relative to its UDS implementation. Beyond raw throughput, TRIC+ offers what Redis cannot: a permutive tier where keys without TTL live in SQLite for free, atomic CAS without Lua scripting (292x faster), and prefix-scan as a first-class operation (65x faster than Redis KEYS).

### Reproduce

```bash
# Build the release binary first — cargo test does not rebuild it.
cargo build --release

# Start a TRIC+ server in the background
./target/release/tric server &

# Run the full benchmark matrix (Redis must be reachable)
REDIS_URL="redis://127.0.0.1/" cargo test --release --test benchmark_test -- --ignored --nocapture
```

## Installation

### As library

```toml
[dependencies]
tric = "0.5.YYMMDDHHMM"  # see latest release
```

### As server

```bash
cargo build --release
TRIC_BASE_DIR=/var/db/tric TRIC_INSTANCE=myapp TRIC_SLOT=0 ./target/release/tric server
```

## Library API

```rust
use tric::{create_tric, Bytes};
use std::time::Duration;

let tric = create_tric();

// Persistent (no TTL = SQLite when used via server)
tric.write_value(b"user:42", b"alice");
assert_eq!(tric.read_value(b"user:42"), Some(Bytes::from_static(b"alice")));

// Transient (TTL = BTreeMap, gone after expiry)
tric.write_value(b"session:abc", b"token");
tric.write_ttl(b"session:abc", Duration::from_secs(3600));

// Prefix scan
let users = tric.find_by_prefix(b"user:");

// Compare-and-delete (atomic job claiming)
tric.delete_value_if_match(b"job:1", b"pending");
```

### Primitives

| Method | Purpose |
|--------|---------|
| `read_value(key)` | Returns value or `None` if absent/expired |
| `write_value(key, value)` | Sets value, clears any existing TTL |
| `delete_value(key)` | Removes key and TTL state. Missing keys: silent no-op |
| `delete_value_if_match(key, expected)` | Deletes only if current value equals `expected` |
| `write_ttl(key, duration)` | Sets expiry on existing key. Missing key: silent no-op |
| `find_by_prefix(prefix)` | Returns all matching `(key, value)` pairs, sorted |

## Server CLI

FreeBSD-style short flags. No GNU long options.

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

Binary protocol over UDS DGRAM (local) and UDP (network). Each network datagram is encrypted with ChaCha20-Poly1305 and padded with random noise — an observer cannot determine operation type or payload size.

| Opcode range | Family |
|--------------|--------|
| `0x01`–`0x07` | Core primitives (read, write, delete, CAS, TTL, scan, query) |
| `0x10`–`0x1A` | Control/Admin (auth, ping, status, shutdown, reload, keys, inspect, dump, restore) |
| `0x80`–`0x81` | Success responses |
| `0x90`–`0x91` | Streaming (scan chunks) |
| `0xA0`–`0xA7` | Error responses |
| `0xB0` | Auth handshake |

## Language bridges

20 languages in four waves. Each bridge undergoes the same quality gate as the core engine.

| Wave | Languages | Mechanism | Ready |
|------|-----------|-----------|-------|
| 1 | **C** | Shared library (.so/.dylib), FFI base | 1/1 |
| 2 | **C++**, **Swift**, Nim, Lua, Tcl, **Zig** | C FFI consumers | 3/6 |
| 3 | PHP, Java, Kotlin, Python, Ruby, C#/.NET, Go | Native socket | 0/7 |
| 4 | JavaScript, TypeScript, Perl, Elixir, Dart, Rust | Native socket | 0/6 |

**Bold** = ready for production use. Remaining: planned.

### CMS/Shop integrations (planned)

WordPress, Drupal, Craft CMS, WooCommerce, Magento, Shopify, PrestaShop, Umbraco — each built on the corresponding Wave 3 bridge.

## Design

**BTreeMap, not HashMap.** Range queries walk contiguous entries. TTL management iterates from oldest expiry. Neither works on a HashMap.

**Lazy expiry.** No background thread. Every operation purges expired entries first. Cost is paid by the next caller, not a separate scheduler.

**Permutive routing.** `write_value` without TTL goes to SQLite. Add `write_ttl` and data moves to BTreeMap. Read from SQLite promotes to BTreeMap cache (60s TTL). The boundary is invisible to the caller.

**Scoped SQLite.** Each namespace (key prefix before `:`) gets its own `.db` file. No cross-table locks. Parallel I/O. WAL mode with `NORMAL` synchronous.

**FreeBSD-first CLI.** Single-letter flags, terse output, `usage:` format. No GNU long options in the interface. Defaults follow FreeBSD filesystem conventions (`/var/db/`, `/var/run/`).

## Size

~3,500 lines of Rust across 20 source files. Single binary ~5 MB (SQLite bundled). 34 tests including server integration with persistence roundtrip. 9 benchmark tests.

## Licence

Dual model. Use whichever fits.

**Server (`tric` crate — binary and library)** is licensed under the **Business Source License 1.1**. Non-production use is unrestricted. Production use on a single host is granted free of charge under the Additional Use Grant; a "host" is one OS instance (one bare-metal machine, VM, container, or FreeBSD jail each count as one). Production across more than one host, or offering TRIC+ as a managed service to third parties, requires a commercial licence. The server converts to Apache-2.0 on **2030-04-19**, or four years after the release date of each tagged version, whichever comes first. See [`LICENSE`](LICENSE) for the full text and [`LICENSE-APACHE`](LICENSE-APACHE) for the Change License.

**Language bridges (`bridges/`)** are licensed under the **BSD 3-Clause License**. No production restriction, no Change Date — use them with any TRIC+ server, any application, any deployment model. See [`bridges/LICENSE`](bridges/LICENSE).

Copyright (c) 2025-2026 Vivian Voss. For commercial licensing, contact <https://vivianvoss.net/tric>.
