# C Bridge: Quickstart

The TRIC+ C client is a single `.c` and `.h` pair with zero dependencies beyond POSIX. It speaks the TRIC+ wire protocol over UDS DGRAM, which means your C program talks to the permutive engine directly: write with a TTL and the value lives in the transient `BTreeMap` tier; write without one and it lives in SQLite. The bridge does not have to know which tier will hold the data, and neither do you.

## Files

| File | Purpose |
|------|---------|
| `tric.h` | Public API: types and function declarations |
| `tric.c` | Implementation: socket handling, wire protocol encoding and decoding |

## Build

```bash
cc -Wall -Wextra -Wpedantic -std=c11 -O2 -c tric.c -o tric.o
```

Link against your application:

```bash
cc -Wall -std=c11 -O2 -o myapp myapp.c tric.o
```

Or build as a shared library:

```bash
cc -shared -fPIC -O2 -o libtric.so tric.c          # Linux / FreeBSD
cc -shared -fPIC -O2 -o libtric.dylib tric.c        # macOS
```

No external dependencies. No `cmake`. No `pkg-config`. Two files, one compiler call.

## Connect

```c
#include "tric.h"

TricConnection connection = create_connection("/var/run/tric/server.sock");
if (!check_connection(&connection)) {
    /* handle error */
}

/* ... use the connection ... */

delete_connection(&connection);
```

The client binds a temporary socket at `/tmp/tric-c-{pid}.sock` and connects to the server. `delete_connection` cleans up both the file descriptor and the temporary socket file.

## Primitives

### Write and read

```c
write_value(&connection,
    (const uint8_t *)"user:42", 7,
    (const uint8_t *)"alice", 5);

TricValue value = read_value(&connection, (const uint8_t *)"user:42", 7);
if (value.data) {
    /* value.data points to "alice", value.length is 5 */
    delete_value_result(&value);
}
```

`read_value` returns a `TricValue`. If the key does not exist, `data` is `NULL`. The caller owns the memory and must call `delete_value_result` to free it.

### Delete

```c
delete_value(&connection, (const uint8_t *)"user:42", 7);
```

Returns `0` on success, `-1` on communication error. Deleting a missing key succeeds silently.

### Compare-and-delete

```c
int matched = delete_value_if_match(&connection,
    (const uint8_t *)"job:1", 5,
    (const uint8_t *)"pending", 7);
/* matched == 1: deleted. matched == 0: value did not match. */
```

Atomic: if the current value equals `expected`, the key is deleted and the function returns `1`. Otherwise the key is untouched and the function returns `0`.

### TTL

```c
write_value(&connection,
    (const uint8_t *)"session:abc", 11,
    (const uint8_t *)"token", 5);

write_ttl(&connection, (const uint8_t *)"session:abc", 11, 3600000);
/* key expires in 3600000 ms (1 hour) */
```

`write_ttl` sets a time-to-live on an existing key. Duration is in milliseconds. A missing key is a silent no-op.

### Prefix scan

```c
TricScanResult scan = find_by_prefix(&connection, (const uint8_t *)"user:", 5);

for (size_t i = 0; i < scan.count; i++) {
    /* scan.pairs[i].key, scan.pairs[i].key_length */
    /* scan.pairs[i].value, scan.pairs[i].value_length */
}

delete_scan_result(&scan);
```

Returns all key-value pairs whose key starts with the given prefix. The caller owns the result and must call `delete_scan_result` to free all allocated memory.

## Types

| Type | Fields | Purpose |
|------|--------|---------|
| `TricConnection` | `socket_fd`, `request_counter` | Connection handle |
| `TricValue` | `data`, `length` | Single value result (caller-owned) |
| `TricPair` | `key`, `key_length`, `value`, `value_length` | One key-value pair |
| `TricScanResult` | `pairs`, `count` | Scan result (caller-owned) |

## Error handling

All functions that return `int` use `0` for success, `-1` for communication error. `read_value` returns `NULL` data on missing key or error. `check_connection` returns non-zero if the connection is valid.

The bridge does not retry. The caller decides whether to reconnect and retry.

## Test

Test source lives in `tests/bridge_test.c`. Build and run:

```bash
cc -Wall -Wextra -Wpedantic -std=c11 -O2 -o tests/bridge_test tric.c tests/bridge_test.c
./tests/bridge_test /path/to/server.sock
```

The test binary exercises all six primitives against a running TRIC+ server and reports pass/fail counts.

## Next

- [Client Overview](../00-overview.md): the wire protocol from the client perspective, plus the minimum API surface every bridge must provide.
- [Wire Protocol](../../server/04-wire-protocol.md): the full opcode reference, including request and response formats for every primitive.
