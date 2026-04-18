# Installation

TRIC+ ships as a single binary. Download it or build it, place it, run it.

## Requirements

- **SQLite** is bundled — no external dependency
- **Operating systems:** FreeBSD, Debian, Ubuntu, RHEL, Rocky, Alpine, Amazon Linux, NixOS, macOS
- **Rust 1.75+** — only required when building from source

## Install a prebuilt binary

Every release publishes platform binaries as assets. Pick the archive that matches your system.

| Platform | Archive |
|----------|---------|
| macOS (Apple Silicon) | `tric-macos-arm64.tar.gz` |
| Linux x86_64 | `tric-linux-x86_64.tar.gz` |
| Linux arm64 | `tric-linux-arm64.tar.gz` |
| FreeBSD x86_64 | `tric-freebsd-x86_64.tar.gz` |

Intel Macs are no longer a published target — build from source if you need one.

```bash
# Replace the archive name with the one for your platform.
VERSION="v0.5.2604181454"
ARCHIVE="tric-linux-x86_64.tar.gz"

curl -LO "https://github.com/VivianVossNet/TRICplus/releases/download/${VERSION}/${ARCHIVE}"
tar -xzf "${ARCHIVE}"
```

The archive contains a single executable named after the platform (for example `tric-linux-x86_64`). Rename it to `tric` and move on to [Deploy](#deploy):

```bash
mv tric-linux-x86_64 tric
chmod +x tric
```

If you have the GitHub CLI, the same thing in one line:

```bash
gh release download v0.5.2604181454 --repo VivianVossNet/TRICplus --pattern 'tric-linux-x86_64.tar.gz'
```

## Build from source

```bash
git clone https://github.com/VivianVossNet/TRICplus.git
cd TRICplus
cargo build --release
```

The binary lands at `target/release/tric`. It is statically linked against SQLite and weighs approximately 5 MB.

## Verify the build

```bash
./target/release/tric -h
./target/release/tric -b
```

The benchmark (`-b`) runs without a server and confirms the engine works on your platform. See [Performance](10-performance.md) for expected numbers.

## Deploy

Copy the binary to a location in your `PATH`. The source path depends on how you obtained it — `./tric` after a download, `target/release/tric` after a source build.

```bash
# FreeBSD, Linux, macOS
install -m 0755 ./tric /usr/local/bin/tric
```

## Start the server

TRIC+ creates its socket and data directories automatically on startup. If running as a non-root user, ensure the parent directories exist and are writable, or set `TRIC_SOCKET_DIR` and `TRIC_BASE_DIR` to paths the user can write to.

See [Configuration](02-configuration.md) for all paths and environment variables.

```bash
tric server
```

By default, TRIC+ listens on:
- **UDS:** `/var/run/tric/server.sock` (local clients)
- **UDP:** `0.0.0.0:7483` (network clients)
- **Admin:** `/var/run/tric/admin.sock` (CLI commands)

Data is stored under `/var/db/tric/default_0/`.

## FreeBSD rc.d service

Create `/usr/local/etc/rc.d/tric`:

```sh
#!/bin/sh

# PROVIDE: tric
# REQUIRE: NETWORKING
# KEYWORD: shutdown

. /etc/rc.subr

name="tric"
rcvar="tric_enable"
command="/usr/local/bin/tric"
command_args="server"
pidfile="/var/run/tric/tric.pid"

tric_user="tric"
tric_env="TRIC_BASE_DIR=/var/db/tric TRIC_INSTANCE=default TRIC_SLOT=0"

load_rc_config $name
run_rc_command "$1"
```

Enable and start:

```bash
sysrc tric_enable=YES
service tric start
```

## Linux systemd unit

Create `/etc/systemd/system/tric.service`:

```ini
[Unit]
Description=TRIC+ Permutive Database Engine
After=network.target

[Service]
Type=simple
User=tric
Group=tric
ExecStart=/usr/local/bin/tric server
Environment=TRIC_BASE_DIR=/var/db/tric
Environment=TRIC_INSTANCE=default
Environment=TRIC_SLOT=0
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
systemctl enable tric
systemctl start tric
```

## Next

- [Configuration](02-configuration.md) — environment variables, paths, defaults
- [CLI Reference](03-cli-reference.md) — every command at your fingertips
