# Storage Model

TRIC+ is a Permutive Database Engine. Data permutes between two storage tiers based on a single signal: TTL.

## The two tiers

| Tier | Backing store | Lifetime | Access speed |
|------|--------------|----------|-------------|
| **Transient** | BTreeMap (in-memory) | Until TTL expires or process exits | ~200ns per operation |
| **Persistent** | SQLite (on-disk, WAL mode) | Survives restarts | ~5–70us per operation |

## Routing rule

The routing logic is one sentence:

> **`write_value` without TTL goes to SQLite. `write_value` followed by `write_ttl` stays in BTreeMap.**

There is no configuration, no annotation, no tier selector. The developer sets a TTL or doesn't. That decision determines where the data lives.

### Write paths

```
write_value("session:abc", token)
write_ttl("session:abc", 3600s)
→ BTreeMap (transient, gone after 1 hour)

write_value("user:42", "alice")
→ SQLite (persistent, survives restart)
```

### Promotion on write_ttl

If a key already exists in SQLite and you call `write_ttl` on it, the data **moves** from SQLite to BTreeMap:

```
write_value("config:theme", "dark")     → SQLite
write_ttl("config:theme", 300s)         → moves to BTreeMap (300s cache)
```

After the TTL expires, the data is gone from BTreeMap. It is also gone from SQLite because the move deleted it. If you want temporary caching of persistent data, read it — cache-promotion handles that automatically.

## Cache-promotion

When you read a key that lives in SQLite, TRIC+ automatically promotes it to BTreeMap with a 60-second cache TTL:

```
read_value("user:42")
  → BTreeMap miss
  → SQLite hit → return value
  → copy to BTreeMap with TTL=60s
  
read_value("user:42")     (within 60s)
  → BTreeMap hit → return value (no SQLite access)
```

This means frequently-read persistent data automatically lives at transient-tier speed. After 60 seconds without access, the cached copy expires and the next read goes to SQLite again.

## Scoped SQLite

Each key namespace gets its own SQLite database file. The namespace is the portion of the key before the first `:`.

| Key | Namespace | SQLite file |
|-----|-----------|-------------|
| `users:42` | `users` | `users.db` |
| `orders:1` | `orders` | `orders.db` |
| `_schema:users` | `_schema` | `_schema.db` |
| `config` | `_default` | `_default.db` |

Benefits:
- **No cross-table locks** — writing to `users.db` never blocks `orders.db`
- **Parallel I/O** — the OS distributes reads and writes across files
- **Independent lifecycle** — drop a namespace by deleting its `.db` file

All SQLite databases use WAL (Write-Ahead Logging) mode with `NORMAL` synchronous, balancing durability and throughput.

## Delete semantics

`delete_value` removes the key from **both** tiers:

```
delete_value("user:42")
  → removes from BTreeMap (if present)
  → removes from SQLite (if present)
```

`delete_value_if_match` applies the same dual-tier logic, but only if the value matches the expected value. This is atomic per tier.

## Prefix scan

`find_by_prefix` queries **both** tiers, merges the results, and deduplicates by key (BTreeMap takes precedence). Results are sorted by key.

```
find_by_prefix("user:")
  → BTreeMap results: [user:42 (cached)]
  → SQLite results:   [user:42, user:99]
  → merged:           [user:42 (from BTreeMap), user:99 (from SQLite)]
```

## Next

- [SQL Interface](06-sql-interface.md) — query persistent data with SQL syntax
- [Instance Management](08-instance-management.md) — slots and cloning
