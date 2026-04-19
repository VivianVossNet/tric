# Client Overview

This document is the bridge implementor's guide. It describes the TRIC+ wire protocol from the client's perspective and defines what every language bridge must provide.

TRIC+ is a permutive database: data lives in a `BTreeMap` in memory when it carries a time-to-live, and in SQLite on disk when it does not. A bridge does not need to know that. The bridge's only job is to speak the wire protocol; routing between tiers happens on the server. Every language ends up with the same six primitives, expressed in the idiom its developers expect.

## What a bridge does

A TRIC+ bridge connects an application in language X to a running TRIC+ server. The bridge:

1. Opens a Unix datagram socket (UDS DGRAM) or UDP socket.
2. Encodes operations as binary datagrams (5-byte header plus length-prefixed fields).
3. Sends one datagram per operation and receives one response.
4. Decodes the response opcode and payload.

That is the entire contract. No connection pooling, no session management for local clients, no retry logic. Each datagram is independent.

## Bridge architecture

There are two categories of bridges, matching the rollout waves.

### Wave 2: C FFI consumers

These languages link against the C bridge (`bridges/c/tric.{c,h}`), which handles encoding, decoding, and socket management. The language wrapper calls C functions via FFI.

**Languages:** C++, Swift, Nim, Lua, Tcl, Zig. All six are ready today.

### Waves 3 and 4: native socket clients

These languages implement the wire protocol directly using their native socket API. No C dependency, no FFI.

**Wave 3:** PHP, Java, Kotlin, Python, Ruby, C#/.NET, Go. Enterprise and web ecosystems.
**Wave 4:** JavaScript, TypeScript, Perl, Elixir, Dart, Rust. Specialised native-socket clients.

## Minimum API surface

Every bridge must expose these functions, adapted to the target language's conventions:

| Function | Wire opcode | Purpose |
|----------|-------------|---------|
| `connect(socket_path)` | n/a | Open the UDS DGRAM connection |
| `disconnect()` | n/a | Close the socket |
| `read_value(key)` | `0x01` | Read a value by key |
| `write_value(key, value)` | `0x02` | Write a key-value pair |
| `delete_value(key)` | `0x03` | Delete a key |
| `delete_value_if_match(key, expected)` | `0x04` | Conditional delete |
| `write_ttl(key, duration_ms)` | `0x05` | Set TTL on an existing key |
| `find_by_prefix(prefix)` | `0x06` | Prefix scan |

Optional but recommended:

| Function | Wire opcode | Purpose |
|----------|-------------|---------|
| `query(sql)` | `0x07` | SQL-subset query |
| `ping()` | `0x13` | Health check |
| `status()` | `0x14` | Server metrics |

## Encoding a request

### Local (UDS) datagram format

```
Byte 0–3:   request_id    (u32, big-endian)
Byte 4:     opcode        (u8)
Byte 5+:    fields        (opcode-specific)
```

### Field encoding

Every variable-length field is prefixed with its length as a `u32` big-endian:

```
[u32 BE length][bytes]
```

### Example: write_value("user:42", "alice")

```python
request_id = 1
opcode = 0x02
key = b"user:42"
value = b"alice"

datagram = (
    request_id.to_bytes(4, 'big') +     # 00 00 00 01
    bytes([opcode]) +                     # 02
    len(key).to_bytes(4, 'big') + key +   # 00 00 00 07 + "user:42"
    len(value).to_bytes(4, 'big') + value # 00 00 00 05 + "alice"
)
```

## Decoding a response

```
Byte 0–3:   request_id    (u32, big-endian). Matches the request.
Byte 4:     opcode        (u8). Response type, see table below.
Byte 5+:    payload       (opcode-specific)
```

### Response opcodes

| Opcode | Meaning | Payload |
|--------|---------|---------|
| `0x80` | OK, no data | *(empty)* |
| `0x81` | OK, with data | `[value_len u32][value]` |
| `0x90` | Scan chunk | `[total u16][chunk_id u16][key_len u32][key][value_len u32][value]` |
| `0x91` | Scan end | *(empty)* |
| `0xA1` | Error: malformed | UTF-8 error message |
| `0xA2` | Error: invalid opcode | *(empty)* |

## Handling scan responses

`find_by_prefix` (opcode `0x06`) returns multiple datagrams:

1. Zero or more `0x90` (scan chunk) datagrams, each containing one key-value pair.
2. One `0x91` (scan end) datagram.

The client must `recv()` in a loop until it receives `0x91`. Each `0x90` contains `total` (total number of chunks) and `chunk_id` (this chunk's index), which allows the client to order the results if datagrams arrive out of sequence.

## Quality requirements

Every bridge undergoes the same quality gate as the TRIC+ core:

- **Five-coder review:** Claudine (KISS and QS), Whitehat (security), Master42 (architectural elimination), Speedy Gonzales (performance), Oldman (product vision).
- **Full Hafenrundfahrt** before merge: language-native formatter, strict compile with warnings as errors, integration test suite against a live server, and the core Rust Hafenrundfahrt as a regression firewall.
- **Integration tests** against a running TRIC+ server, matching the reference 14-check coverage of the C bridge.
- **Package-manager distribution** where the target language has one (crates.io, PyPI, npm, Packagist, luarocks, and so on).

## Language-specific manuals

As each bridge is implemented, its manual is added under `clients/{language}/`:

```
clients/
├── 00-overview.md          (this file)
├── c/01-quickstart.md
├── cpp/01-quickstart.md
├── swift/01-quickstart.md
├── nim/01-quickstart.md
├── lua/01-quickstart.md
├── tcl/01-quickstart.md
├── zig/01-quickstart.md
└── (more as bridges ship)
```

## Next

- [Wire Protocol](../server/04-wire-protocol.md): full opcode reference for every request and response type.
- [Storage Model](../server/05-storage-model.md): how the transient and persistent tiers interact, so you understand what the bridge connects to.
