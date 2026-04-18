# Import and Export

TRIC+ can ingest SQL dumps, export data as SQL or `.tric` archives, and synchronise incrementally between snapshots.

## SQL import

### Basic import

```bash
tric import -f database.sql -F mysql
```

TRIC+ parses the SQL dump, analyses the schema (CREATE TABLE statements), and writes each row as a key-value pair. The analyser applies seven deterministic rules to map relational structure to KV:

1. Primary key columns become the key suffix
2. Non-primary columns become the newline-delimited value
3. Foreign keys produce `_rel:` relationship entries
4. Table schemas are stored as `_schema:{table}`
5. Column types are preserved in the schema metadata
6. TTL candidates are identified (columns named `expires_at`, `ttl`, etc.)
7. Table names are sanitised for use as key prefixes

### Supported dialects

| Flag | Dialect |
|------|---------|
| `-F mysql` | MySQL (backtick-quoted identifiers) |
| `-F postgres` | PostgreSQL (double-quoted identifiers) |
| `-F sqlite` | SQLite (no quoting) |

### Analyse mode

Preview the storage plan without writing data:

```bash
tric import -f database.sql -F mysql -a
```

This shows which tables will be created, how keys will be structured, and where relationships will be stored.

## Diff-import

Compare two `.tric` snapshots and apply only the differences:

```bash
tric import -D yesterday.tric today.tric
```

The diff engine:
1. Reads both archives into memory (HashMap by tar path)
2. Entries in `new` but not in `old` → written (addition)
3. Entries in both with different content → overwritten (modification)
4. Entries in `old` but not in `new` → deleted (deletion)
5. Identical entries → skipped

TTL entries (`_ttl/*` paths) are applied alongside their data entries. Schema and relationship entries are diffed the same way.

Output:

```
12 additions, 3 modifications, 1 deletions applied.
```

### Use cases

- **Incremental backup restore** — apply only what changed since the last snapshot
- **Staging sync** — diff production against staging, apply the delta
- **Migration testing** — diff pre-migration against post-migration, verify changes

## .tric export

### Compressed (default)

```bash
tric export -f backup.tric
```

Produces a Brotli-compressed tar archive. Internal structure:

```
_meta/version           → "tric+1"
_schema/{table}         → schema metadata
_rel/{relationship}     → relationship markers
{table}/{key}           → value
_ttl/{table}/{key}      → TTL in milliseconds (string)
```

### Debug (uncompressed)

```bash
tric export -f debug.tric -d
```

Same structure, uncompressed tar. Useful for inspection with standard tools:

```bash
tar tf debug.tric
tar xf debug.tric -C /tmp/inspect/
```

## SQL export

```bash
tric export -f backup.sql -F postgres
```

Generates `CREATE TABLE` and `INSERT INTO` statements in the specified dialect. Type mapping adapts to the target dialect (e.g., `BOOLEAN` becomes `TINYINT(1)` for MySQL, `INTEGER` for SQLite).

## Binary dump and restore

For raw key-value backup without schema interpretation:

```bash
# Dump
tric dump -f store.bin

# Restore
tric restore -f store.bin
```

The binary format stores each entry as: `[key_len u32][key][value_len u32][value][ttl_ms u64]`. TTL values of 0 indicate no TTL (persistent).

## Next

- [Instance Management](08-instance-management.md) — slots and cloning
- [Storage Model](05-storage-model.md) — where imported data lands
