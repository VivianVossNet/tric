# Instance Management

TRIC+ supports multiple projects and project clones on a single installation. Each project is an **instance**. Each instance has numbered **slots** for versioning, staging, and migration testing.

## Concepts

### Instance

An instance is a named project. Set via `TRIC_INSTANCE` (default: `default`). Each instance gets its own set of SQLite databases under `TRIC_BASE_DIR`.

### Slot

A slot is a numbered copy of an instance's data. Slot `_0` is always the primary. Higher-numbered slots (`_1`, `_2`, ...) are clones.

### Layout

```
/var/db/tric/                   # TRIC_BASE_DIR
├── webshop_0/                  # instance "webshop", slot 0 (primary)
│   ├── users.db
│   ├── orders.db
│   └── _schema.db
├── webshop_1/                  # clone of slot 0 (staging)
│   ├── users.db
│   ├── orders.db
│   └── _schema.db
└── analytics_0/                # different instance
    ├── events.db
    └── _schema.db
```

## Instance registry

On startup, TRIC+ scans `TRIC_BASE_DIR` and populates a registry in the transient layer:

```
_instance:webshop_0      → active
_instance:webshop_1      → clone:webshop_0
_instance:analytics_0    → active
```

The registry enables runtime discovery of all instances and their relationships. Query it with:

```bash
tric keys -p _instance:
```

## Managing slots

### List slots

```bash
$ TRIC_INSTANCE=webshop tric slots
  webshop_0  4821504B  (primary)
  webshop_1  4821504B
```

### Create a clone

```bash
$ TRIC_INSTANCE=webshop tric clone 1
cloned webshop_0 → webshop_1  (4821504B)
```

This copies all `.db` files from the source slot to the target. The clone is an independent copy — changes to one do not affect the other.

### Switch active slot

Start the server with a different `TRIC_SLOT`:

```bash
TRIC_INSTANCE=webshop TRIC_SLOT=1 tric server
```

### Delete a slot

Remove the directory:

```bash
rm -rf /var/db/tric/webshop_1
```

## Use cases

### Staging environment

```bash
# Clone production to staging
TRIC_INSTANCE=myapp tric clone 1

# Run staging server on the clone
TRIC_INSTANCE=myapp TRIC_SLOT=1 TRIC_SOCKET_DIR=/var/run/tric-staging tric server
```

### Migration testing

```bash
# Clone before migration
TRIC_INSTANCE=myapp tric clone 2

# Apply migration to the clone
TRIC_INSTANCE=myapp TRIC_SLOT=2 TRIC_SOCKET_DIR=/var/run/tric-test tric server &
TRIC_SOCKET_DIR=/var/run/tric-test tric import -f migration.sql -F postgres
TRIC_SOCKET_DIR=/var/run/tric-test tric shutdown

# Verify — if good, switch production to the migrated slot
# If bad, delete the clone and start over
```

### Snapshot before risky operations

```bash
TRIC_INSTANCE=myapp tric clone 3    # safety net
tric import -f big-change.sql -F mysql
# If something goes wrong: TRIC_SLOT=3 tric server
```

## Next

- [Authentication](09-authentication.md) — securing network access
- [Configuration](02-configuration.md) — all environment variables
