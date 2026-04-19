# Zig Bridge: Quickstart

The TRIC+ Zig client is a standard Zig package that wraps the C bridge via `@cImport`. It exposes `tric.Connection`: a struct with explicit-allocator semantics, Zig-native error unions, and the six TRIC+ primitives. The Zig wrapper compiles the C source as part of the package build, so there are no pre-built libraries to manage and no system dependencies beyond POSIX. Permutive routing stays on the server: a `write` followed by `ttl` lives in the transient `BTreeMap`, a plain `write` lives in SQLite, and the Zig code sees one API.

## Requirements

- **Zig 0.16+** (`zig version` to check)
- A running TRIC+ server reachable via a Unix-domain socket (build with `cargo build --release`)

## Integration

Add the Zig bridge as a dependency in your `build.zig.zon`:

```zig
.{
    .name = .myapp,
    .version = "0.0.0",
    .minimum_zig_version = "0.16.0",
    .dependencies = .{
        .tric = .{ .path = "/path/to/TRIC/bridges/zig" },
    },
    .paths = .{"build.zig", "build.zig.zon", "src"},
}
```

Wire it into your `build.zig`:

```zig
const tric_dep = b.dependency("tric", .{
    .target = target,
    .optimize = optimize,
});
exe.root_module.addImport("tric", tric_dep.module("tric"));
```

Then `const tric = @import("tric");` in your Zig source.

## Connect

```zig
const std = @import("std");
const tric = @import("tric");

pub fn main() !void {
    var gpa: std.heap.GeneralPurposeAllocator(.{}) = .init;
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    var connection = tric.Connection.init(allocator, "/var/run/tric/server.sock");
    defer connection.deinit();

    if (!connection.isValid()) {
        return error.ConnectionFailed;
    }

    // ... use the connection ...
}
```

`Connection.init` binds a temporary socket at `/tmp/tric-c-{pid}.sock` and connects to the server. `deinit` closes the file descriptor and removes the temporary socket. Zig has no destructors, and the explicit `defer connection.deinit();` makes cleanup visible at the call site.

## Primitives

### Write and read

```zig
try connection.write("user:42", "alice");

if (try connection.read("user:42")) |value| {
    defer allocator.free(value);
    // value is []u8 containing "alice"
}
```

`read` returns `!?[]u8`, an error union over a nullable slice. `null` means the key is absent; a non-null result is caller-owned memory allocated from the connection's allocator. Free with `allocator.free(value)`.

### Delete

```zig
try connection.del("user:42");
```

Returns `!void`. Throws `error.Communication` on socket failure. Deleting a missing key succeeds silently.

### Compare-and-delete

```zig
const matched = try connection.cad("job:1", "pending");
// matched == true:  the value was "pending", key is now deleted
// matched == false: the value was something else, key is untouched
```

Returns `!bool`. Distinguishes "value mismatch = false" from "communication error = error". Atomic on the server side.

### TTL

```zig
try connection.write("session:abc", "token");
try connection.ttl("session:abc", 3_600_000);
// key expires in 3 600 000 ms (1 hour)
```

Sets time-to-live on an existing key. Missing key is a silent no-op. Returns `!void`.

### Prefix scan

```zig
const pairs = try connection.scan("user:");
defer tric.freeScan(allocator, pairs);

for (pairs) |pair| {
    // pair.key and pair.value are []u8, owned by the returned slice
}
```

Returns `![]tric.Pair` where each `Pair` is `{ key: []u8, value: []u8 }`. All memory is caller-owned; `tric.freeScan(allocator, pairs)` frees every `key`, `value`, and the outer slice in one call.

## API

| Method | Signature | Purpose |
|--------|-----------|---------|
| `init(allocator, socket_path)` | `fn init(std.mem.Allocator, [:0]const u8) Connection` | Open a connection |
| `deinit(*Connection)` | `fn deinit(*Connection) void` | Close the socket; idempotent |
| `isValid(*Connection)` | `fn isValid(*Connection) bool` | Check if the connection is live |
| `read(*Connection, key)` | `fn read(*Connection, []const u8) !?[]u8` | Fetch a value; null if absent |
| `write(*Connection, key, value)` | `fn write(*Connection, []const u8, []const u8) !void` | Store a value |
| `del(*Connection, key)` | `fn del(*Connection, []const u8) !void` | Remove a key |
| `cad(*Connection, key, expected)` | `fn cad(*Connection, []const u8, []const u8) !bool` | Atomic compare-and-delete |
| `ttl(*Connection, key, duration_ms)` | `fn ttl(*Connection, []const u8, u64) !void` | Set expiry on an existing key |
| `scan(*Connection, prefix)` | `fn scan(*Connection, []const u8) ![]Pair` | Fetch all pairs by key prefix |

| Free function | Signature | Purpose |
|---------------|-----------|---------|
| `freeScan(allocator, pairs)` | `fn freeScan(std.mem.Allocator, []Pair) void` | Free a scan result (keys, values, and the outer slice) |

## Error handling

The bridge returns `Error` from `src/root.zig`:

- `error.ConnectionInvalid`: operations attempted on an invalidly-opened connection. Reserved; all current methods succeed or return `error.Communication`.
- `error.Communication`: socket failure. The send call errored, the recv call errored, or the server returned an error opcode.
- `error.OutOfMemory`: the allocator could not satisfy a read or scan allocation.

Absent values are `null` in the `?[]u8` union, not errors. `cad` mismatch is `false`, not an error. The bridge does not retry. The caller decides whether to reconnect.

## Memory model

All caller-facing byte containers are `[]u8` allocated from the allocator passed to `Connection.init`. The connection stores a reference to the allocator; all `read` / `scan` allocations come from it. Free with the same allocator:

- `read` returns `?[]u8`; call `allocator.free(value)` after use.
- `scan` returns `[]Pair`; call `tric.freeScan(allocator, pairs)` to free every key, every value, and the outer slice in one call.

Zig's explicit allocator policy makes ownership unambiguous. The bridge never allocates behind the caller's back.

## Test

Tests live at `tests/bridge_test.zig` and assume a running TRIC+ server reachable via the socket at `$TRIC_SOCKET` (default `/tmp/tric-zig-test/server.sock`). Start a scratch server, run `zig build test`, tear it down:

```bash
# Build the TRIC+ server binary
cargo build --release

# Start a scratch server on a temporary socket
mkdir -p /tmp/tric-zig-test
TRIC_SOCKET_DIR=/tmp/tric-zig-test \
TRIC_BASE_DIR=/tmp/tric-zig-test/data \
TRIC_INSTANCE=zigtest TRIC_SLOT=0 \
./target/release/tric server &
SERVER_PID=$!
sleep 2

# Run the Zig test suite
TRIC_SOCKET=/tmp/tric-zig-test/server.sock \
zig build test --package-path bridges/zig

# Tear down
kill $SERVER_PID
rm -rf /tmp/tric-zig-test
```

The test binary exercises all six primitives plus a varied-slice round-trip, 14 test blocks in total, against the running server.

## Next

- [C Bridge Quickstart](../c/01-quickstart.md) : the underlying C layer that every Wave-2 bridge consumes via FFI
- [C++ Bridge Quickstart](../cpp/01-quickstart.md) : the C++ RAII wrapper, a sibling Wave-2 FFI consumer
- [Swift Bridge Quickstart](../swift/01-quickstart.md) : the Swift SPM package (sibling Wave-2 FFI consumer)
- [Client Overview](../00-overview.md) : the wire protocol from the client perspective, plus the minimum API surface every bridge must provide
- [Wire Protocol](../../server/04-wire-protocol.md) : the full opcode reference, including request and response formats for every primitive
