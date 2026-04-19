# Nim Bridge: Quickstart

The TRIC+ Nim client is a standard nimble package that wraps the C bridge via `{.importc.}` and `{.compile.}` pragmas. It exposes `TricConnection`: a value type with automatic `=destroy` cleanup, Nim-native `string` and `seq[tuple]` returns, and idiomatic exception-based error handling. The Nim layer compiles the C source inline, so there are no pre-built libraries to manage. Permutive routing happens on the server: a `write` followed by `ttl` lives in the `BTreeMap`, a plain `write` lives in SQLite, and the Nim code talks to one API.

## Requirements

- **Nim 2.0+** (`nim --version` to check)
- A running TRIC+ server reachable via a Unix-domain socket (build with `cargo build --release`)

## Integration

Add the Nim bridge as a dependency in your `yourpkg.nimble`:

```nim
requires "tric >= 0.0.0"
```

Until a nimble registry entry exists, use a local path dependency (`requires "tric"` + a `lock` entry pointing at the TRIC+ checkout), or vendor the `bridges/nim/` directory into your project and `import tric`.

## Connect

```nim
import tric
import std/options

var connection = initConnection("/var/run/tric/server.sock")

if not connection.isValid():
  echo "failed to connect"
  quit(1)

# ... use the connection ...
# `=destroy` runs automatically when `connection` goes out of scope;
# the socket closes and the temporary client socket file is removed.
```

`initConnection` binds a temporary socket at `/tmp/tric-c-{pid}.sock` and connects to the server. Nim 2.x runs `=destroy` automatically at scope exit; there is no explicit close call.

## Primitives

### Write and read

```nim
connection.write("user:42", "alice")

let value = connection.read("user:42")
if value.isSome():
  echo value.get()  # "alice"
```

`read` returns `Option[string]`. An empty optional means the key does not exist, or the read failed; the bridge does not distinguish the two cases.

### Delete

```nim
connection.del("user:42")
```

Raises `TricError` on socket failure. Deleting a missing key succeeds silently.

### Compare-and-delete

```nim
let matched = connection.cad("job:1", "pending")
# matched == true:  value was "pending", key is now deleted
# matched == false: value was something else, key is untouched
```

Atomic. Returns `bool`; raises `TricError` on communication failure (distinguishes "mismatch = false" from "comm error = exception").

### TTL

```nim
connection.write("session:abc", "token")
connection.ttl("session:abc", 3_600_000'u64)
# key expires in 3 600 000 ms (1 hour)
```

Duration in milliseconds as `uint64`. Missing key is a silent no-op.

### Prefix scan

```nim
let pairs = connection.scan("user:")
for (key, value) in pairs:
  echo key, " = ", value
```

Returns `seq[tuple[key, value: string]]`. Each pair's `key` and `value` are owned `string`s; no manual cleanup required.

## API

| Procedure | Signature | Purpose |
|-----------|-----------|---------|
| `initConnection(socketPath)` | `proc initConnection(socketPath: string): TricConnection` | Open a connection |
| `isValid` | `proc isValid(conn: var TricConnection): bool` | Check if the connection is live |
| `read` | `proc read(conn: var TricConnection, key: string): Option[string]` | Fetch a value; `none` if absent |
| `write` | `proc write(conn: var TricConnection, key, value: string)` | Store a value |
| `del` | `proc del(conn: var TricConnection, key: string)` | Remove a key |
| `cad` | `proc cad(conn: var TricConnection, key, expected: string): bool` | Atomic compare-and-delete |
| `ttl` | `proc ttl(conn: var TricConnection, key: string, durationMs: uint64)` | Set expiry on an existing key |
| `scan` | `proc scan(conn: var TricConnection, prefix: string): seq[TricPair]` | Fetch all pairs by prefix |

`TricPair` is `tuple[key, value: string]`.

## Error handling

The Nim bridge uses idiomatic Nim error semantics:

- **Absent values** → `Option[T]` (`read` returns `none(string)` for missing keys)
- **Communication failures** → `raise TricError` (`write`, `del`, `ttl` raise on socket error)
- **`cad` value mismatch** → returns `false` (mismatch is not an error)

`TricConnection` is non-copyable (`=copy` is marked `{.error.}`). Pass by `var`, or share via a higher-level container. `=destroy` is the only lifecycle hook the caller needs to know about; it runs automatically.

## Test

Tests live at `tests/bridge_test.nim` (nimble convention) and assume a running TRIC+ server reachable via the socket at `$TRIC_SOCKET` (default `/tmp/tric-nim-test/server.sock`). Start a scratch server, run `nimble test`, tear it down:

```bash
# Build the TRIC+ server binary
cargo build --release

# Start a scratch server
mkdir -p /tmp/tric-nim-test
TRIC_SOCKET_DIR=/tmp/tric-nim-test \
TRIC_BASE_DIR=/tmp/tric-nim-test/data \
TRIC_INSTANCE=nimtest TRIC_SLOT=0 \
./target/release/tric server &
SERVER_PID=$!
sleep 2

# Run the Nim test suite
cd bridges/nim
TRIC_SOCKET=/tmp/tric-nim-test/server.sock nimble test

# Tear down
kill $SERVER_PID
rm -rf /tmp/tric-nim-test
```

The test suite exercises all six primitives plus a varied-string round-trip: 14 test blocks via `std/unittest`.

## Next

- [C Bridge Quickstart](../c/01-quickstart.md) : the underlying C layer that every Wave-2 bridge consumes via FFI
- [C++ Bridge Quickstart](../cpp/01-quickstart.md) : the C++ RAII wrapper
- [Swift Bridge Quickstart](../swift/01-quickstart.md) : the Swift SPM package
- [Zig Bridge Quickstart](../zig/01-quickstart.md) : the Zig `build.zig` package
- [Client Overview](../00-overview.md) : the wire protocol from the client perspective, plus the minimum API surface every bridge must provide
- [Wire Protocol](../../server/04-wire-protocol.md) : the full opcode reference, including request and response formats for every primitive
