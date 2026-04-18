# CLI Reference

TRIC+ follows FreeBSD conventions: single-letter flags, terse output, no GNU long options.

## Commands

### Server management

#### `tric server`

Starts the TRIC+ daemon. Binds UDS sockets and UDP, spawns worker threads, enters the supervision loop. Does not daemonise — use your service manager for that.

```bash
tric server
```

The server logs to syslog in BSD RFC 3164 format.

#### `tric shutdown`

Sends a shutdown command to the running server via the admin socket.

```bash
tric shutdown
```

#### `tric status`

Displays server metrics: request counts, error rate, active sessions, latency.

```bash
$ tric status
tric-server
  requests  1842 total 1839 local 3 network
  errors    0
  sessions  0
  latency   42us avg 891us max
```

### Data operations

#### `tric keys [-p prefix]`

Lists keys in the store. Without `-p`, lists all keys.

```bash
# All keys
$ tric keys
users:42  5B
users:99  7B
orders:1  12B

# Filter by prefix
$ tric keys -p users:
users:42  5B
users:99  7B
```

#### `tric inspect <key>`

Displays metadata for a specific key: size, TTL remaining, storage tier.

```bash
$ tric inspect users:42
key     users:42
size    5B
ttl     none (persistent)
tier    transient
```

#### `tric query <SQL>`

Executes a SQL-subset query against the store. See [SQL Interface](06-sql-interface.md) for the supported syntax.

```bash
# Point lookup
$ tric query "SELECT * FROM users WHERE key = '42'"
alice

# Insert
$ tric query "INSERT INTO users VALUES (99, 'bob')"
OK

# Prefix scan
$ tric query "SELECT * FROM users WHERE key LIKE '4%'"
users:42  5B

# Full table scan
$ tric query "SELECT * FROM users"
users:42  5B
users:99  3B
```

### Import and export

#### `tric import -f <path> -F <dialect> [-a]`

Imports a SQL dump file. Supported dialects: `mysql`, `postgres`, `sqlite`.

| Flag | Purpose |
|------|---------|
| `-f <path>` | Path to the SQL dump file |
| `-F <dialect>` | SQL dialect (`mysql`, `postgres`, `sqlite`) |
| `-a` | Analyse only — show the storage plan without importing |

```bash
# Import a MySQL dump
$ tric import -f database.sql -F mysql
3 tables, 1042 rows, 5 relationships imported. 0 errors.

# Dry run — see what would be imported
$ tric import -f database.sql -F mysql -a
```

#### `tric import -D <old.tric> <new.tric>`

Diff-import: compares two `.tric` snapshots and applies only the changes (additions, modifications, deletions).

```bash
$ tric import -D yesterday.tric today.tric
12 additions, 3 modifications, 1 deletions applied.
```

See [Import and Export](07-import-export.md) for details on the diff algorithm.

#### `tric export -f <path> [-d] [-F <dialect>]`

Exports data from the store.

| Flag | Purpose |
|------|---------|
| `-f <path>` | Output file path |
| `-d` | Debug mode — uncompressed tar instead of Brotli |
| `-F <dialect>` | Export as SQL instead of `.tric` format |

```bash
# Export as .tric (Brotli-compressed tar)
$ tric export -f backup.tric
142 entries exported to backup.tric (Brotli)

# Export as uncompressed tar (for inspection)
$ tric export -f debug.tric -d
142 entries exported to debug.tric (uncompressed tar)

# Export as SQL
$ tric export -f backup.sql -F postgres
84 rows exported to backup.sql (postgres)
```

### Binary dump and restore

#### `tric dump -f <path>`

Dumps the entire store to a binary file (key-value pairs with TTL data).

```bash
$ tric dump -f store.bin
142 entries  28904B  written to store.bin
```

#### `tric restore -f <path>`

Restores a binary dump into the running store.

```bash
$ tric restore -f store.bin
142 entries restored from store.bin
```

### Instance management

#### `tric slots`

Lists all slots for the current instance.

```bash
$ TRIC_INSTANCE=webshop tric slots
  webshop_0  4821504B  (primary)
  webshop_1  4821504B
```

#### `tric clone <slot>`

Clones the current slot to a new slot number. Copies all SQLite database files.

```bash
$ tric clone 1
cloned webshop_0 → webshop_1  (4821504B)
```

See [Instance Management](08-instance-management.md) for the full slot lifecycle.

### Utilities

#### `tric -b`

Runs the built-in performance benchmark. Tests transient, persistent, and cache-promoted operations. Optionally compares against a running Redis and TRIC+ server.

```bash
# Engine benchmark only
$ tric -b

# With Redis comparison
$ REDIS_URL="redis://127.0.0.1/" tric -b

# With server comparison (start server first)
$ tric server &
$ tric -b
```

See [Performance](10-performance.md) for methodology and expected numbers.

#### `tric shell`

Interactive REPL. Type commands as you would on the command line, without the `tric` prefix. Exit with `exit` or `quit`.

```bash
$ tric shell
tric> status
tric-server
  requests  0 total 0 local 0 network
  ...
tric> keys -p users:
users:42  5B
tric> exit
```

#### `tric -h`

Prints the usage summary.

## Next

- [Wire Protocol](04-wire-protocol.md) — binary protocol for programmatic clients
- [Storage Model](05-storage-model.md) — how data flows between tiers
