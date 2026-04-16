# TRIC

An embedded Rust key-value store with TTLs and prefix queries. Thread-safe via `Arc<RwLock<Store>>`; fewer than 200 lines of source.

TRIC — Transient Relational Indexed Content — exposes six primitives (read, write, delete, compare-and-delete, TTL, prefix scan) behind a single `Tric` type that clones cheaply across threads. The store lives inside your process and disappears when it exits. That is the design, not a limitation.

## Installation

```toml
[dependencies]
tric = "0.1.0-alpha.1"
```

## API

```rust
pub fn create_tric() -> Tric;
```

Creates a new, empty store. The returned `Tric` wraps an internal `Arc<RwLock<Store>>`; `Tric: Clone` returns a cheap handle on the same store.

```rust
impl Tric {
    pub fn read_value(&self, key: &[u8]) -> Option<Bytes>;
    pub fn write_value(&self, key: &[u8], value: &[u8]);
    pub fn delete_value(&self, key: &[u8]);
    pub fn delete_value_if_match(&self, key: &[u8], expected: &[u8]) -> bool;
    pub fn write_ttl(&self, key: &[u8], duration: Duration);
    pub fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)>;
}
```

- `read_value` returns the value for `key`, or `None` if absent or expired.
- `write_value` sets the value for `key`. Any existing TTL on that key is cleared.
- `delete_value` removes `key` together with its TTL state. Missing keys: no-op.
- `delete_value_if_match` is Compare-and-**Delete**, not Compare-and-Swap. It deletes `key` only if its current value equals `expected`, and returns `true` on deletion, `false` otherwise.
- `write_ttl` sets an expiry duration for an existing `key`. If `key` is not present, the call is a silent no-op. A subsequent `write_value` on the same key clears the TTL.
- `find_by_prefix` returns every non-expired `(key, value)` pair whose key starts with `prefix`, in key order.

Arguments at the public boundary are `&[u8]`; return values use `bytes::Bytes`, re-exported from the crate root. Cloning a `Bytes` is O(1) — reference-count only.

## Usage

### Basic

```rust
use tric::{create_tric, Bytes};
use std::time::Duration;

let tric = create_tric();
tric.write_value(b"user:42", b"alice");
assert_eq!(tric.read_value(b"user:42"), Some(Bytes::from_static(b"alice")));

tric.write_ttl(b"user:42", Duration::from_secs(60));
assert_eq!(tric.find_by_prefix(b"user:").len(), 1);

tric.delete_value(b"user:42");
assert_eq!(tric.read_value(b"user:42"), None);
```

### Supervision pattern

A supervisor thread observes module heartbeats and reclaims crashed slots atomically. Each module registers itself, then refreshes a short TTL at a regular interval. When a module fails to refresh, its entry expires; the supervisor sees the absence on the next scan. Reclamation uses `delete_value_if_match` so a delayed module waking up late cannot steal a slot from its replacement.

```rust
use tric::{create_tric, Tric};
use std::thread;
use std::time::Duration;

fn run_module(tric: Tric, name: &'static [u8]) {
    tric.write_value(name, b"running");
    loop {
        tric.write_ttl(name, Duration::from_secs(30));
        thread::sleep(Duration::from_secs(10));
    }
}

fn run_supervisor(tric: Tric) {
    loop {
        thread::sleep(Duration::from_secs(5));
        let _alive = tric.find_by_prefix(b"module:");
    }
}

let tric = create_tric();
thread::spawn({
    let handle = tric.clone();
    move || run_module(handle, b"module:ingest")
});
thread::spawn({
    let handle = tric.clone();
    move || run_supervisor(handle)
});
```

Modules communicate only through TRIC; they hold no direct references to each other. The pattern scales to any number of modules in a single process.

## Design decisions

**BTreeMap, not HashMap.** The internal store uses `BTreeMap<Bytes, Bytes>`. Range queries over a key namespace walk contiguous entries. TTL management is an ordered iteration from the oldest expiry, touching only expired keys. Neither is expressible efficiently on a HashMap.

**Lazy expiry.** No background thread, no timer. Every public method calls an internal `delete_expired_entries(Instant::now())` before its own work. Keys that expire while no one is watching stay invisible to any subsequent reader; the cost of their removal is paid by the next caller, not by a separate scheduler.

**`Arc<RwLock<Store>>` concurrency.** `Tric::clone()` is a cheap Arc-clone. Every method acquires a write lock for the duration of one operation. The lock is held briefly. The Rust compiler proves the absence of data races at compile time; the lock provides serialisation of concurrent operations.

**No persistence.** State lives in memory for the lifetime of the process. No WAL, no snapshot, no recovery. Use-cases that require durability (audit trails, user data, job queues across restarts) belong in a database, not here.

**`bytes::Bytes` for keys and values.** One allocation per distinct value. Cloning is O(1) — reference-count only. The return path of `read_value` and `find_by_prefix` produces owned values without copying the underlying data.

**CAS is Compare-and-Delete, not Compare-and-Swap.** `delete_value_if_match(key, expected)` removes the entry iff its current value equals `expected`. Ownership transfers by deletion, not mutation. This is the correct primitive for job-queue semantics: a second worker cannot steal the same job because "delete only if value is still X" is atomic.

## Scope

TRIC is:

- an embedded in-process library
- a key-value store with optional TTL and prefix queries
- thread-safe via `Arc<RwLock<Store>>` and cheap clone-handles

TRIC is not:

- a database — no persistence, no transactions, no schema
- a message broker — no subscribers, no queues, no delivery guarantees
- a network service — no server, no wire protocol, no RPC
- a serialisation format — values are raw bytes, interpretation is the caller's concern

For persistence, SQLite. For inter-process messaging, NATS or Kafka. For typed storage, serde layered on top. TRIC's scope will not expand to include these.

## Size

The implementation sits in two source files:

- `src/store.rs` — 95 lines
- `src/lib.rs` — 62 lines

Total: 157 lines. The design budget is ~800 lines; integration tests add another ~380 lines across eight files.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).
