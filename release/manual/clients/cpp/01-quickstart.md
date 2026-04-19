# C++ Bridge: Quickstart

The TRIC+ C++ client is a single header-only wrapper (`tric.hpp`) built on top of the C bridge. It provides an idiomatic RAII API with `std::optional`, `std::string_view`, and `std::vector`, compiles to the same machine code as the raw C calls, and builds without exceptions.

## Requirements

- **C++17** compiler (GCC 8+, Clang 7+, MSVC 2017+)
- The TRIC+ C bridge sources (`bridges/c/tric.h`, `bridges/c/tric.c`)

The C++ layer is a pure inline wrapper. There is no separate `.cpp` implementation file.

## Files

| File | Purpose |
|------|---------|
| `tric.hpp` | C++ public API: `tric::connection` RAII class, six primitive methods |
| `tric.h`   | C header from `bridges/c/`: forward-declared types and function prototypes |
| `tric.c`   | C implementation from `bridges/c/`: socket handling, wire-protocol encoding and decoding |

## Build

Compile the C implementation once with a C compiler:

```bash
cc -Wall -Wextra -Wpedantic -std=c11 -O2 -c tric.c -o tric.o
```

Then build your C++ application against it:

```bash
c++ -Wall -Wextra -Wpedantic -std=c++17 -O2 -o myapp myapp.cpp tric.o
```

`tric.hpp` and `tric.h` must be on the include path (or sitting next to your source). No `cmake`, no `pkg-config`, no dependencies beyond POSIX and the C++17 standard library.

## Connect

```cpp
#include "tric.hpp"

tric::connection connection("/var/run/tric/server.sock");
if (!connection) {
    // handle error
}

// ... use the connection ...

// destructor closes the socket and cleans up the temporary client socket file
```

The connection is RAII: the constructor binds a temporary socket at `/tmp/tric-c-{pid}.sock` and connects to the server; the destructor closes the file descriptor and removes the temporary socket file. `connection` is non-copyable and move-only.

## Primitives

### Write and read

```cpp
connection.write("user:42", "alice");

if (auto value = connection.read("user:42")) {
    // *value == "alice"
}
```

`read` returns `std::optional<std::string>`. An empty optional means the key does not exist, or the read failed; the bridge does not distinguish the two cases. The returned `std::string` owns its bytes and can hold arbitrary octets including embedded nulls.

### Delete

```cpp
bool ok = connection.del("user:42");
```

Returns `true` on success, `false` on communication error. Deleting a missing key succeeds silently.

### Compare-and-delete

```cpp
bool matched = connection.cad("job:1", "pending");
// matched == true: deleted. matched == false: value did not match.
```

Atomic: if the current value equals the expected value, the key is deleted and the method returns `true`. Otherwise the key is untouched and the method returns `false`.

### TTL

```cpp
connection.write("session:abc", "token");
connection.ttl("session:abc", 3'600'000);
// key expires in 3 600 000 ms (1 hour)
```

`ttl` sets a time-to-live on an existing key. Duration is in milliseconds. A missing key is a silent no-op. Returns `true` on successful acknowledgement.

### Prefix scan

```cpp
auto pairs = connection.scan("user:");
for (auto& [key, value] : pairs) {
    // key and value are std::string, both owned
}
```

Returns `std::vector<std::pair<std::string, std::string>>` containing all key-value pairs whose key starts with the given prefix. The vector owns its contents; no manual cleanup required.

## API

| Method | Signature | Purpose |
|--------|-----------|---------|
| `connection(path)` | `explicit connection(std::string_view)` | Open a connection to the UDS socket |
| `valid()` / `operator bool()` | `bool valid() const` | Check if the connection is live |
| `read(key)` | `std::optional<std::string> read(std::string_view)` | Fetch a value; empty if absent |
| `write(key, value)` | `bool write(std::string_view, std::string_view)` | Store a value |
| `del(key)` | `bool del(std::string_view)` | Remove a key |
| `cad(key, expected)` | `bool cad(std::string_view, std::string_view)` | Atomic compare-and-delete |
| `ttl(key, duration_ms)` | `bool ttl(std::string_view, std::uint64_t)` | Set expiry on an existing key |
| `scan(prefix)` | `std::vector<std::pair<std::string, std::string>> scan(std::string_view)` | Fetch all pairs by key prefix |

## Error handling

The bridge is **exception-free**. No method throws, allocates unexpectedly, or aborts on error. Failure paths:

- `read`: an empty `std::optional` means the key is absent or the read failed.
- `write`, `del`, `cad`, `ttl`: return `false` on communication error. For `cad`, `false` also means the stored value did not match the expected value.
- `scan`: returns an empty vector on error or when no keys match the prefix.

The bridge does not retry. The caller decides whether to reconnect and retry. Design rationale: many C++ deployments build with `-fno-exceptions` (games, embedded, kernel modules, latency-sensitive servers). Keeping the bridge exception-free makes it universally usable.

## RAII semantics

- `tric::connection` is **non-copyable** (`= delete` on copy constructor and copy assignment)
- `tric::connection` is **move-only**. Move construction and move assignment transfer ownership; the moved-from connection becomes inert.
- The destructor closes the socket unconditionally, so you can rely on it.

```cpp
tric::connection a("/var/run/tric/server.sock");
tric::connection b(std::move(a));
// a is now inert; b owns the socket
```

## Test

Test source lives in `tests/bridge_test.cpp` and includes `tric.hpp` from the parent directory. Build and run (run from `bridges/cpp/`):

```bash
cc  -Wall -Wextra -Wpedantic -std=c11   -O2 -c ../c/tric.c -o tric_c.o
c++ -Wall -Wextra -Wpedantic -std=c++17 -O2 -I ../c -I . -o tests/bridge_test tests/bridge_test.cpp tric_c.o
./tests/bridge_test /path/to/server.sock
```

`-I ../c` finds `tric.h`; `-I .` finds `tric.hpp` (one directory up from the test source).

The test binary exercises all six primitives plus move-construction semantics against a running TRIC+ server and reports pass/fail counts.

## Next

- [C Bridge Quickstart](../c/01-quickstart.md) : the underlying C layer that every Wave-2 bridge consumes via FFI
- [Client Overview](../00-overview.md) : the wire protocol from the client perspective, plus the minimum API surface every bridge must provide
- [Wire Protocol](../../server/04-wire-protocol.md) : the full opcode reference, including request and response formats for every primitive
