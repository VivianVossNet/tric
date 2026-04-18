# Installation

TRIC+ ships as a single binary. Build it, place it, run it.

## Requirements

- **Rust 1.75+** (for building from source)
- **SQLite** is bundled — no external dependency
- **Operating systems:** FreeBSD, Debian, Ubuntu, RHEL, Rocky, Alpine, Amazon Linux, NixOS, macOS

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

Copy the binary to a location in your `PATH`:

```bash
# FreeBSD
cp target/release/tric /usr/local/bin/tric

# Linux
cp target/release/tric /usr/local/bin/tric

# macOS
cp target/release/tric /usr/local/bin/tric
```

## Create directories

TRIC+ needs two directories: one for sockets, one for data.

```bash
# FreeBSD / Linux
mkdir -p /var/run/tric /var/db/tric
chown tric:tric /var/run/tric /var/db/tric

# macOS (development)
mkdir -p /tmp/tric-sockets /tmp/tric-data
```

See [Configuration](02-configuration.md) for all paths and environment variables.

## Start the server

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
