# Swift Bridge — Quickstart

The TRIC+ Swift client is a Swift Package Manager (SPM) package that wraps the C bridge with an idiomatic, exception-free Swift API. It exposes `Tric.Connection` — a RAII class with `throws`-based error handling, `Data`-primary inputs, and `String` convenience overloads. The Swift layer compiles to the same machine code as the raw C calls; there is no runtime overhead beyond the FFI.

## Requirements

- **Swift 5.9+** (Xcode 15+ on macOS; Swift toolchain 5.9+ on Linux)
- **macOS 13+** or **Ubuntu 22.04+**

## Integration

Add the Swift bridge as a dependency in your `Package.swift`:

```swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MyApp",
    dependencies: [
        .package(path: "/path/to/TRIC/bridges/swift")
    ],
    targets: [
        .executableTarget(
            name: "MyApp",
            dependencies: [
                .product(name: "Tric", package: "swift")
            ]
        )
    ]
)
```

For local development, `path:` points at the Swift bridge directory inside the TRIC+ checkout. Pre-built packaging (tagged releases on a Swift registry) will follow in a later release.

## Connect

```swift
import Tric

let connection = Connection(socketPath: "/var/run/tric/server.sock")
guard connection.isValid else {
    // handle error
    return
}

// ... use the connection ...

// Destructor runs on last release of `connection` — the socket closes and
// the temporary client socket file is removed.
```

`Connection` is a `final class` with RAII semantics. The constructor binds a temporary socket at `/tmp/tric-c-{pid}.sock` and connects to the server; `deinit` closes the file descriptor and removes the temporary socket file.

## Primitives

### Write and read

```swift
try connection.write("user:42", "alice")

if let data = connection.read("user:42"),
   let name = String(data: data, encoding: .utf8) {
    // name == "alice"
}
```

`read` returns `Data?`. A `nil` result means the key does not exist (or the read failed — bridges do not distinguish). The returned `Data` owns its bytes and can hold arbitrary octets, including embedded nulls.

### Delete

```swift
try connection.del("user:42")
```

Throws `TricError.communication` on socket failure. Deleting a missing key succeeds silently.

### Compare-and-delete

```swift
let matched = try connection.cad("job:1", expected: "pending")
// matched == true:  value was "pending", key is now deleted
// matched == false: value was something else, key is untouched
```

Atomic: if the current value equals the expected value, the key is deleted and the method returns `true`. Otherwise the key is untouched and the method returns `false`. Throws `TricError.communication` on socket failure — distinguishes "mismatch" (returns `false`) from "communication error" (throws).

### TTL

```swift
try connection.write("session:abc", "token")
try connection.ttl("session:abc", durationMs: 3_600_000)
// key expires in 3 600 000 ms (1 hour)
```

`ttl` sets a time-to-live on an existing key. Duration is in milliseconds. A missing key is a silent no-op. Throws `TricError.communication` on socket failure.

### Prefix scan

```swift
let pairs = connection.scan("user:")
for (key, value) in pairs {
    // key, value are Data — convert to String if needed
}
```

Returns `[(Data, Data)]` containing all key-value pairs whose key starts with the given prefix. The array owns its contents; no manual cleanup required.

## API

| Method | Signature | Purpose |
|--------|-----------|---------|
| `init(socketPath:)` | `init(socketPath: String)` | Open a connection to the UDS socket |
| `isValid` | `var isValid: Bool` | Check if the connection is live |
| `read(_:)` | `func read(_ key: Data) -> Data?` | Fetch a value; `nil` if absent |
| `read(_:)` | `func read(_ key: String) -> Data?` | String-keyed convenience overload |
| `write(_:_:)` | `func write(_ key: Data, _ value: Data) throws` | Store a value |
| `write(_:_:)` | `func write(_ key: String, _ value: String) throws` | String-keyed convenience overload |
| `del(_:)` | `func del(_ key: Data) throws` | Remove a key |
| `del(_:)` | `func del(_ key: String) throws` | String-keyed convenience overload |
| `cad(_:expected:)` | `func cad(_ key: Data, expected: Data) throws -> Bool` | Atomic compare-and-delete |
| `cad(_:expected:)` | `func cad(_ key: String, expected: String) throws -> Bool` | String-keyed convenience overload |
| `ttl(_:durationMs:)` | `func ttl(_ key: Data, durationMs: UInt64) throws` | Set expiry on an existing key |
| `ttl(_:durationMs:)` | `func ttl(_ key: String, durationMs: UInt64) throws` | String-keyed convenience overload |
| `scan(_:)` | `func scan(_ prefix: Data) -> [(Data, Data)]` | Fetch all pairs by key prefix |
| `scan(_:)` | `func scan(_ prefix: String) -> [(Data, Data)]` | String-keyed convenience overload |

## Error handling

The Swift bridge uses idiomatic Swift error semantics:

- **Absent values** → `Optional` (`read`, `scan` return `nil` / empty for missing data)
- **Communication failures** → `throws TricError` (`write`, `del`, `cad`, `ttl` throw `.communication` on socket error)
- **`cad` value mismatch** → returns `false` (mismatch is not an error, it is the expected non-match path)

Swift's `throws` is a zero-cost return flag (not exception-based), so exception-free builds (unlike in C++) are not a concern.

## RAII semantics

- `Connection` is a `class` — reference type with deterministic `deinit`
- Copying the reference shares the underlying socket; `deinit` runs when the last reference releases
- Passing across threads requires your own synchronisation — the bridge itself is not thread-safe (the C layer uses a single `request_counter` per connection without atomics)

For thread safety, either serialise calls on your own actor / queue, or open one `Connection` per thread / actor.

## Test

Tests live at `Tests/TricTests/BridgeTest.swift` (SPM's standard layout). Run against a built TRIC+ server:

```bash
# Build the TRIC+ server binary first
cargo build --release

# Run the Swift tests (the harness finds ./target/release/tric automatically,
# or set TRIC_BINARY to an absolute path)
TRIC_BINARY=$(pwd)/target/release/tric swift test --package-path bridges/swift
```

The test harness starts a scratch `tric server` on a temporary UDS path, runs 14 tests covering all six primitives plus `String` convenience overloads, and tears down the server and temporary directory on completion.

## Next

- [C Bridge Quickstart](../c/01-quickstart.md) — the underlying C layer
- [C++ Bridge Quickstart](../cpp/01-quickstart.md) — the C++ RAII wrapper (sibling Wave-2 FFI consumer)
- [Client Overview](../00-overview.md) — wire protocol from the client perspective
- [Wire Protocol](../../server/04-wire-protocol.md) — full opcode reference
