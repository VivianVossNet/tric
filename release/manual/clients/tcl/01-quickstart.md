# Tcl Bridge — Quickstart

The TRIC+ Tcl client is a loadable C extension (`libtric_tcl.dylib` / `.so`) that exposes the six TRIC+ primitives as `tric::*` Tcl commands. Scripts `package require tric` and call the commands; the extension takes care of the C bridge underneath.

## Requirements

- **Tcl 9.0+** (`tclsh` from `brew install tcl-tk` on macOS; packaged distributions on Linux)
- A C compiler (cc / clang / gcc) with Tcl development headers available
- A running TRIC+ server reachable via a Unix-domain socket

## Build

From the repo root:

```bash
bridges/tcl/build.sh
```

The script auto-detects the Tcl install prefix (default `/opt/homebrew/opt/tcl-tk` on macOS; override with `TCLTK_PREFIX`). Output: `bridges/tcl/libtric_tcl.dylib` (macOS) or `.so` (Linux/BSD).

## Load and connect

```tcl
lappend auto_path /path/to/TRIC/bridges/tcl
package require tric

set h [tric::connect "/var/run/tric/server.sock"]
if {![tric::valid $h]} {
    puts "connect failed"
    exit 1
}

# ... use the connection ...

tric::disconnect $h
```

`tric::connect` returns a handle token (opaque string like `trich1`). Pass the handle to all subsequent `tric::*` commands. Call `tric::disconnect` when done — no automatic cleanup, Tcl has no destructor hook.

## Primitives

### Write and read

```tcl
tric::write $h "user:42" "alice"

set value [tric::read $h "user:42"]
if {$value ne ""} {
    puts "got: $value"
}
```

`tric::read` returns the value as a byte array (usable as a string for UTF-8 content), or the empty string `""` if the key is absent.

### Delete

```tcl
tric::del $h "user:42"
```

Silently succeeds if the key is missing. Raises a Tcl error on communication failure (catch with `catch { ... }`).

### Compare-and-delete

```tcl
set matched [tric::cad $h "job:1" "pending"]
# matched == 1: value was "pending", key is now deleted
# matched == 0: value was something else, key is untouched
```

Returns `1` or `0`. Raises on comm failure.

### TTL

```tcl
tric::write $h "session:abc" "token"
tric::ttl $h "session:abc" 3600000
```

Duration in milliseconds (Tcl integer / wide integer). Missing key is a silent no-op.

### Prefix scan

```tcl
set pairs [tric::scan $h "user:"]
foreach {key value} $pairs {
    puts "$key = $value"
}
```

Returns a flat Tcl list: `{key1 value1 key2 value2 …}`. Consume with `foreach {k v} $pairs { … }`.

## Commands

| Command | Args | Result |
|---------|------|--------|
| `tric::connect` | socketPath | handle token or error |
| `tric::disconnect` | handle | — |
| `tric::valid` | handle | 1 / 0 |
| `tric::read` | handle key | value (byte-array) or "" |
| `tric::write` | handle key value | — or error |
| `tric::del` | handle key | — or error |
| `tric::cad` | handle key expected | 1 / 0, or error |
| `tric::ttl` | handle key durationMs | — or error |
| `tric::scan` | handle prefix | flat list {k v k v …} |

## Error handling

Communication failures raise Tcl errors; catch with:

```tcl
if {[catch {tric::write $h $k $v} err]} {
    puts "write failed: $err"
}
```

Absent values (read of a missing key) return empty string. `cad` mismatch returns `0`, not an error.

## Test

Tests live at `bridges/tcl/tests/bridge_test.tcl` (tcltest). Start a scratch server, load the freshly-built extension, run the test:

```bash
cargo build --release

mkdir -p /tmp/tric-tcl-test
TRIC_SOCKET_DIR=/tmp/tric-tcl-test \
TRIC_BASE_DIR=/tmp/tric-tcl-test/data \
TRIC_INSTANCE=tcltest TRIC_SLOT=0 \
./target/release/tric server &
SERVER_PID=$!
sleep 2

bridges/tcl/build.sh   # if not already built
TRIC_SOCKET=/tmp/tric-tcl-test/server.sock \
/opt/homebrew/opt/tcl-tk/bin/tclsh bridges/tcl/tests/bridge_test.tcl

kill $SERVER_PID
rm -rf /tmp/tric-tcl-test
```

The test suite exercises all six primitives plus a varied-bytes round-trip — 14 `tcltest` checks.

## Next

- [C Bridge Quickstart](../c/01-quickstart.md) — underlying C layer
- [Client Overview](../00-overview.md) — wire protocol from the client perspective
- [Wire Protocol](../../server/04-wire-protocol.md) — full opcode reference
