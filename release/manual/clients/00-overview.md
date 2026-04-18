# Client Overview

This document is the bridge implementor's guide. It describes the TRIC+ wire protocol from the client's perspective and defines what every language bridge must provide.

## What a bridge does

A TRIC+ bridge connects an application in language X to a running TRIC+ server. The bridge:

1. Opens a Unix datagram socket (UDS DGRAM) or UDP socket
2. Encodes operations as binary datagrams (5-byte header + length-prefixed fields)
3. Sends one datagram per operation, receives one response
4. Decodes the response opcode and payload

That's it. No connection pooling, no session management (for local), no retry logic required. Each datagram is independent.

## Bridge architecture

There are two categories of bridges:

### C FFI bridges (Wave 2)

These languages link against the C shared library (`libtric`), which handles encoding, decoding, and socket management. The language wrapper calls C functions via FFI.

**Languages:** C++, Swift, Nim, Lua, Tcl, Zig

### Native socket bridges (Waves 3–4)

These languages implement the wire protocol directly using their native socket API. No C dependency.

**Languages:** PHP, Java, Kotlin, Python, Ruby, C#/.NET, Go, JavaScript, TypeScript, Perl, Elixir, Dart, Rust

## Minimum API surface

Every bridge must expose these functions (adapted to language conventions):

| Function | Wire opcode | Purpose |
|----------|-------------|---------|
| `connect(socket_path)` | — | Open UDS DGRAM connection |
| `disconnect()` | — | Close socket |
| `read_value(key)` | `0x01` | Read a value by key |
| `write_value(key, value)` | `0x02` | Write a key-value pair |
| `delete_value(key)` | `0x03` | Delete a key |
| `delete_value_if_match(key, expected)` | `0x04` | Conditional delete |
| `write_ttl(key, duration_ms)` | `0x05` | Set TTL on existing key |
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

Every variable-length field is prefixed with its length as a u32 big-endian:

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
Byte 0–3:   request_id    (u32, big-endian) — matches the request
Byte 4:     opcode        (u8) — response type
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

1. Zero or more `0x90` (scan chunk) datagrams, each containing one key-value pair
2. One `0x91` (scan end) datagram

The client must `recv()` in a loop until it receives `0x91`. Each `0x90` contains `total` (total number of chunks) and `chunk_id` (this chunk's index) for ordering.

## Quality requirements

Every bridge undergoes the same quality gate as the TRIC+ core:

- **Three-coder review:** Claudine (KISS/QS), Whitehat (Security), M45T3R42 (Efficiency)
- **Full Hafenrundfahrt** before merge
- **Integration tests** against a running TRIC+ server
- **Package manager distribution** (where applicable)

## Language-specific manuals

As each bridge is implemented, its manual is added under `clients/{language}/`:

```
clients/
├── 00-overview.md          (this file)
├── rust/
│   └── 01-quickstart.md
├── c/
│   └── 01-quickstart.md
├── php/
│   └── 01-quickstart.md
└── ...
```

## Next

- [Wire Protocol](../server/04-wire-protocol.md) — full opcode reference
- [Storage Model](../server/05-storage-model.md) — understand what the bridge connects to
