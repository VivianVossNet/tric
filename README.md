# TRIC+

**Permutive Database Engine** — data permutes between transient memory and persistent storage based on lifetime, not configuration.

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

## Performance

Measured on two systems. Redis running on the same machine for comparison. All values are single-threaded, 128-byte payloads.

### macOS (Apple Silicon)

| Workload | TRIC+ | Redis (TCP) | Factor |
|----------|------:|------------:|-------:|
| Transient write | 2,600,000 ops/s | 5,000 ops/s | 500x |
| Transient read | 4,100,000 ops/s | 9,000 ops/s | 455x |
| Cache-promoted read | 4,100,000 ops/s | — | — |
| SQLite write (WAL) | 19,000 ops/s | — | — |

### FreeBSD 15 (AMD Ryzen 5 3600, ZFS, Jail)

| Workload | TRIC+ | Redis (TCP) | Factor |
|----------|------:|------------:|-------:|
| Transient write | 1,600,000 ops/s | 73,000 ops/s | 21x |
| Transient read | 1,800,000 ops/s | 95,000 ops/s | 19x |
| Cache-promoted read | 2,400,000 ops/s | — | — |
| SQLite write (WAL/ZFS) | 15,000 ops/s | — | — |
| Concurrent write (4 threads) | 685,000 ops/s | — | — |

TRIC+ transient operations run in-process — no network hop, no serialisation, no context switch. Redis requires TCP even on localhost. The difference is architectural, not optimisation.

### Reproduce

```bash
# TRIC+ benchmarks (no server required)
cargo test --release --test benchmark_test -- --ignored --nocapture

# With Redis comparison (Redis must be running)
REDIS_URL="redis://127.0.0.1/" cargo test --release --test benchmark_test -- --ignored --nocapture

# With Redis auth
REDIS_URL="redis://:password@host/" cargo test --release --test benchmark_test -- --ignored --nocapture
```

## Installation

### As library

```toml
[dependencies]
tric = "0.1.0-alpha.1"
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

// Persistent (no TTL → SQLite when used via server)
tric.write_value(b"user:42", b"alice");
assert_eq!(tric.read_value(b"user:42"), Some(Bytes::from_static(b"alice")));

// Transient (TTL → BTreeMap, gone after expiry)
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

```bash
tric server                          # start daemon
tric status                          # server metrics
tric keys [-p prefix]                # list keys
tric inspect <key>                   # key metadata + TTL
tric query <SQL>                     # SQL subset (SELECT/INSERT/UPDATE/DELETE)
tric import -f <path> --format mysql|postgres|sqlite [--analyse]
tric import --diff <old.tric> <new.tric>
tric export -f <path.tric> [--debug] [--format mysql|postgres|sqlite]
tric dump -f <path>                  # binary store dump
tric restore -f <path>               # binary store restore
tric slots                           # list instance slots
tric clone <slot>                    # clone current slot
tric shutdown                        # stop server
tric shell                           # interactive REPL
```

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `TRIC_SOCKET_DIR` | `/var/run/tric` | UDS socket directory |
| `TRIC_UDP_BIND` | `0.0.0.0:7483` | UDP listen address |
| `TRIC_BASE_DIR` | `/var/db/tric` | SQLite storage base directory |
| `TRIC_INSTANCE` | `default` | Instance name |
| `TRIC_SLOT` | `0` | Active slot (`_0` = primary, `_1`+ = clones) |

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

## Design

**BTreeMap, not HashMap.** Range queries walk contiguous entries. TTL management iterates from oldest expiry. Neither works on a HashMap.

**Lazy expiry.** No background thread. Every operation purges expired entries first. Cost is paid by the next caller, not a separate scheduler.

**Permutive routing.** `write_value` without TTL goes to SQLite. Add `write_ttl` and data moves to BTreeMap. Read from SQLite promotes to BTreeMap cache (60s TTL). The boundary is invisible to the caller.

**Scoped SQLite.** Each namespace (key prefix before `:`) gets its own `.db` file. No cross-table locks. Parallel I/O. WAL mode with `NORMAL` synchronous.

## Size

~3,200 lines of Rust across 19 source files. Single binary ~5 MB (SQLite bundled). 34 tests including server integration with persistence roundtrip.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).
