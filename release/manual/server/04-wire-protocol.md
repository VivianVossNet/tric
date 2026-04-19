# Wire Protocol

TRIC+ uses a binary datagram protocol. Local clients communicate via UDS DGRAM. Network clients communicate via UDP with per-datagram ChaCha20-Poly1305 encryption.

## Transport

### Local (UDS DGRAM)

Each `send()` on the Unix datagram socket is one request. Each `recv()` is one response. No connection state. No encryption — the operating system's file permissions are the access gate.

```
┌───────────────┬──────────┬──────────┐
│ request_id (4)│opcode (1)│  fields  │
└───────────────┴──────────┴──────────┘
```

Minimum overhead: **5 bytes** (4-byte request ID + 1-byte opcode).

### Network (UDP, encrypted)

Every network datagram is encrypted with ChaCha20-Poly1305. A session key is established via the [authentication handshake](09-authentication.md). Random noise (64–1024 bytes) is appended inside the encrypted block — an observer cannot determine operation type, payload size, or distinguish padded from unpadded traffic.

```
┌─────────────────┬──────────────────────────────────────────────────┬──────────┐
│ session_id (16) │              encrypted block                     │ auth (16)│
│   cleartext     │  request_id(4) + opcode(1) + fields + noise     │ Poly1305 │
└─────────────────┴──────────────────────────────────────────────────┴──────────┘
```

Fixed overhead: 16 (session) + 4 (request_id) + 1 (opcode) + 16 (auth tag) = **37 bytes**.

## Field encoding

All fields use length-prefixed byte strings:

```
[u32 BE length][bytes]
```

Durations use `[u64 BE milliseconds]`.

The `request_id` (u32, big-endian) correlates requests with responses. The client chooses the ID; the server echoes it in the response.

## Opcode table

### Core primitives (client → server)

| Opcode | Name | Request fields | Response |
|--------|------|----------------|----------|
| `0x01` | read_value | `[key_len][key]` | Found: `0x81 [value_len][value]`. Not found: `0x80`. |
| `0x02` | write_value | `[key_len][key][value_len][value][duration_ms u64 BE]` | `0x80` |
| `0x03` | delete_value | `[key_len][key]` | `0x80` |
| `0x04` | delete_value_if_match | `[key_len][key][expected_len][expected]` | Deleted: `0x81 [0x01]`. Not deleted: `0x81 [0x00]`. |
| `0x05` | write_ttl | `[key_len][key][duration_ms u64 BE]` | `0x80` |
| `0x06` | find_by_prefix | `[prefix_len][prefix]` | Stream of `0x90` chunks + `0x91` end. |
| `0x07` | query | `[sql_len][sql]` | Varies by SQL statement. See [SQL Interface](06-sql-interface.md). |

### Control and admin (client → server)

| Opcode | Name | Request fields | Response |
|--------|------|----------------|----------|
| `0x10` | AUTH_INIT | `[ed25519_pubkey 32B][x25519_ephemeral 32B]` | `0xB0` challenge. |
| `0x11` | AUTH_PROOF | `[ed25519_signature 64B]` | Encrypted `0x80` (session established). |
| `0x13` | PING | *(none)* | `0x80` |
| `0x14` | STATUS | *(none)* | `0x81 [7x u64 BE]` — requests total, local, network, errors, sessions, avg latency, max latency. Local only. |
| `0x15` | SHUTDOWN | *(none)* | Server terminates. Local only. |
| `0x16` | RELOAD | *(none)* | `0x80` — reloads authorized_keys. Local only. |
| `0x17` | KEYS | `[prefix_len][prefix]` (optional, empty = all) | Stream of `0x90` chunks + `0x91` end. |
| `0x18` | INSPECT | `[key_len][key]` | Found: `0x81 [value_len][value][ttl_ms u64 BE]`. Not found: `0x80`. |
| `0x19` | DUMP | *(none)* | Stream of `0x90` chunks (with TTL) + `0x91` end. |
| `0x1A` | RESTORE | `[key_len][key][value_len][value][ttl_ms u64 BE]` | `0x80` |

### Response opcodes (server → client)

| Opcode | Meaning |
|--------|---------|
| `0x80` | OK — no payload |
| `0x81` | OK — with payload |
| `0x90` | Scan chunk: `[total u16 BE][chunk_id u16 BE][key_len][key][value_len][value]` |
| `0x91` | Scan end |
| `0xA0` | Error: generic |
| `0xA1` | Error: malformed datagram |
| `0xA2` | Error: invalid opcode |
| `0xA3` | Error: payload too large |
| `0xA5` | Error: authentication required |
| `0xA6` | Error: authentication failed |
| `0xA7` | Error: auth not configured |
| `0xB0` | Auth challenge: `[nonce 32B][x25519_server_ephemeral 32B]` |

## Write paths at a glance

TRIC+ has one write opcode. The trailing `duration_ms` field decides the storage tier — that is the routing signal of K0051 expressed at the wire boundary.

| Field value | Storage tier | Semantics | Use for |
|-------------|--------------|-----------|---------|
| `duration_ms = 0` | Persistent (SQLite) | Long-lived write, no expiry | Configuration, imported tables, anything that must survive restart |
| `duration_ms > 0` | Transient (BTreeMap) | Atomic write + TTL in milliseconds | Sessions, caches, the `SET k v EX t` equivalent |

`0x05 write_ttl` is the complement: it updates the TTL of a key that already exists, and (in PermutiveBus) promotes a persistent row to the transient tier. Use it when you wrote a value first and now want to attach or extend an expiry.

## Worked example

Write `user:42` = `alice` via local UDS, then read it back.

### Write request (persistent — no TTL)

```
00 00 00 01                   request_id: 1
02                            opcode: write_value
00 00 00 07                   key length: 7
75 73 65 72 3a 34 32          key: "user:42"
00 00 00 05                   value length: 5
61 6c 69 63 65                value: "alice"
00 00 00 00 00 00 00 00       duration_ms: 0 (persistent → SQLite)
```

Total: 29 bytes.

### Write request (transient — with TTL)

Same shape, with a non-zero duration:

```
00 00 00 01                   request_id: 1
02                            opcode: write_value
00 00 00 0a                   key length: 10
73 65 73 73 69 6f 6e 3a 61 62 key: "session:ab"
00 00 00 05                   value length: 5
74 6f 6b 65 6e                value: "token"
00 00 00 00 00 00 0e 10       duration_ms: 3600 (3.6 s, transient → BTreeMap)
```

Total: 32 bytes.

### Write response

```
00 00 00 01                   request_id: 1
80                            opcode: OK
```

Total: 5 bytes.

### Read request

```
00 00 00 02                   request_id: 2
01                            opcode: read_value
00 00 00 07                   key length: 7
75 73 65 72 3a 34 32          key: "user:42"
```

### Read response

```
00 00 00 02                   request_id: 2
81                            opcode: OK with payload
00 00 00 05                   value length: 5
61 6c 69 63 65                value: "alice"
```

## Implementing a client

A minimal TRIC+ client needs:

1. **Connect** — bind a UDS DGRAM socket, connect to `server.sock`
2. **Encode** — build the 5-byte header (request_id + opcode) + length-prefixed fields
3. **Send** — one `send()` per request
4. **Receive** — one `recv()` per response, match by request_id
5. **Decode** — parse the response opcode and payload

See [Client Overview](../clients/00-overview.md) for language-specific guidance.

## Next

- [Storage Model](05-storage-model.md) — how the engine routes data
- [Authentication](09-authentication.md) — ed25519 handshake for network clients
