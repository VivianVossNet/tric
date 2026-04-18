# Configuration

TRIC+ is configured entirely through environment variables. No configuration file. No command-line flags beyond the subcommand. Set the variables in your service manager, shell profile, or wrapper script.

## Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `TRIC_SOCKET_DIR` | `/var/run/tric` | Directory for UDS sockets (server.sock, admin.sock) |
| `TRIC_UDP_BIND` | `0.0.0.0:7483` | UDP listen address and port for network clients |
| `TRIC_BASE_DIR` | `/var/db/tric` | Base directory for SQLite persistent storage |
| `TRIC_INSTANCE` | `default` | Instance name — identifies this project |
| `TRIC_SLOT` | `0` | Active storage slot (0 = primary, 1+ = clones) |
| `REDIS_URL` | `redis://127.0.0.1/` | Redis URL for benchmark comparison (used by `tric -b` only) |

## Filesystem layout

TRIC+ creates the following structure on first start:

```
/var/run/tric/                  # TRIC_SOCKET_DIR
├── server.sock                 # UDS DGRAM — client data operations
└── admin.sock                  # UDS DGRAM — CLI admin commands

/var/db/tric/                   # TRIC_BASE_DIR
└── default_0/                  # {TRIC_INSTANCE}_{TRIC_SLOT}
    ├── users.db                # one SQLite file per namespace
    ├── orders.db
    └── _schema.db              # schema metadata
```

### Socket directory

The socket directory must exist before TRIC+ starts. TRIC+ creates the socket files but does not create the directory itself on production paths (it does create it if using a custom `TRIC_SOCKET_DIR`).

Socket permissions are inherited from the directory. Set directory permissions to control access:

```bash
# Only the tric user and group can connect
chmod 0750 /var/run/tric
chown tric:tric /var/run/tric
```

### Data directory

TRIC+ creates the instance subdirectory (`default_0/`) automatically. Each key namespace (the portion before the first `:` in a key) gets its own SQLite database file.

For example, writing keys `users:42`, `users:99`, and `orders:1` produces:

```
/var/db/tric/default_0/
├── users.db        # contains users:42 and users:99
└── orders.db       # contains orders:1
```

See [Storage Model](05-storage-model.md) for the routing logic and [Instance Management](08-instance-management.md) for multiple instances and cloning.

## Multiple instances

Run multiple TRIC+ instances on the same machine by setting different `TRIC_INSTANCE` values:

```bash
# Instance 1: webshop
TRIC_INSTANCE=webshop TRIC_SOCKET_DIR=/var/run/tric-webshop tric server &

# Instance 2: analytics
TRIC_INSTANCE=analytics TRIC_SOCKET_DIR=/var/run/tric-analytics tric server &
```

Each instance needs its own `TRIC_SOCKET_DIR` to avoid socket conflicts. They can share the same `TRIC_BASE_DIR` — the instance name separates the data directories.

## OS-specific defaults

The default paths follow the FreeBSD filesystem hierarchy. On other systems, adjust as needed:

| Purpose | FreeBSD | Linux | macOS (dev) |
|---------|---------|-------|-------------|
| Sockets | `/var/run/tric` | `/var/run/tric` | `/tmp/tric-sockets` |
| Data | `/var/db/tric` | `/var/db/tric` | `/tmp/tric-data` |
| Binary | `/usr/local/bin/tric` | `/usr/local/bin/tric` | `./target/release/tric` |

## Next

- [CLI Reference](03-cli-reference.md) — every command and flag
- [Instance Management](08-instance-management.md) — slots, cloning, multiple projects
