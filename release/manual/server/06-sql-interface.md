# SQL Interface

TRIC+ accepts a subset of SQL that maps cleanly to key-value operations. The parser uses `sqlparser-rs` with the generic dialect, so MySQL, PostgreSQL, and SQLite syntax all work without switching modes.

## Supported statements

### SELECT

```sql
-- Point lookup by key
SELECT * FROM users WHERE key = '42'

-- Prefix scan
SELECT * FROM users WHERE key LIKE 'admin-%'

-- Full table scan
SELECT * FROM users
```

`WHERE key = '...'` maps to `read_value`. `WHERE key LIKE '...%'` maps to `find_by_prefix`. No WHERE clause scans all entries for that table.

### INSERT

```sql
INSERT INTO users VALUES (42, 'alice', 'admin')
INSERT INTO users VALUES (99, 'bob', 'viewer')
```

The first column is always the primary key. The key becomes `{table}:{pk_value}`. Non-primary columns are stored as newline-delimited values.

If no `_schema:{table}` entry exists, one is created automatically with inferred column types.

### UPDATE

```sql
UPDATE users SET role = 'editor' WHERE key = '42'
```

UPDATE requires `WHERE key = ...`. It reads the existing value, replaces it, and writes back. Only modifies existing keys — no upsert.

### DELETE

```sql
DELETE FROM users WHERE key = '42'
```

DELETE requires `WHERE key = ...`. No bulk delete without a WHERE clause.

## Key mapping

SQL tables map to key prefixes:

| SQL concept | KV mapping |
|-------------|-----------|
| Table `users` | Key prefix `users:` |
| Row with PK `42` | Key `users:42` |
| Non-PK columns | Newline-delimited value |
| Schema | `_schema:users` |

## Unsupported syntax

JOINs, subqueries, GROUP BY, ORDER BY, HAVING, aggregate functions, and transactions are not supported. Attempting them returns an error (`0xA1 Malformed`). TRIC+ is a key-value store with SQL convenience, not a relational database.

## Usage

### Via CLI

```bash
tric query "SELECT * FROM users WHERE key = '42'"
tric query "INSERT INTO users VALUES (42, 'alice')"
```

### Via wire protocol

Send opcode `0x07` with the SQL string as a length-prefixed field. See [Wire Protocol](04-wire-protocol.md).

## Next

- [Import and Export](07-import-export.md) — bulk data operations
- [Storage Model](05-storage-model.md) — how the data is stored
