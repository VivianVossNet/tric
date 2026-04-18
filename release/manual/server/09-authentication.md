# Authentication

TRIC+ uses ed25519 public-key authentication for network (UDP) clients. Local clients (UDS) are authenticated by file permissions — no handshake required.

## Why native auth

The Elasticsearch lesson: delegating authentication entirely to an outer layer fails when operators misconfigure that layer or deploy outside the expected perimeter. TRIC+ handles its own security.

## Modes

| Flag | Behaviour |
|------|-----------|
| `--auth-keys <path>` | Required. Path to `authorized_keys.pub`. Missing file = startup abort. |
| `--no-auth` | Disabled. WARNING logged at every startup. |
| Neither flag | Startup abort. |

Authentication is default-on. Disabling it requires an explicit, intentional flag.

## authorized_keys.pub

```
# /etc/tric/authorized_keys.pub
# <label> <base64-ed25519-pubkey>
api-gateway        MCowBQYDK2VwAyEA...
analytics-worker   MCowBQYDK2VwAyEA...
```

The server loads this file at startup and reloads it on `RELOAD` (opcode `0x16` or `tric reload`). Active sessions whose public key is removed are terminated immediately.

## Handshake

Three datagrams establish a session:

```
Client                                  Server
  │                                       │
  │── AUTH_INIT (0x10) ──────────────────→│
  │   [ed25519_pubkey 32B]                │  lookup in authorized_keys.pub
  │   [x25519_ephemeral_pubkey 32B]       │  generate ephemeral X25519 keypair
  │                                       │  generate 32B nonce
  │←── AUTH_CHALLENGE (0xB0) ────────────│
  │   [nonce 32B]                         │
  │   [x25519_server_ephemeral 32B]       │
  │                                       │
  │── AUTH_PROOF (0x11) ─────────────────→│
  │   [ed25519_signature 64B]             │  verify signature over nonce
  │                                       │  derive shared secret (X25519)
  │                                       │  create session entry
  │←── OK (encrypted) ──────────────────│
  │   [session_id 16B][encrypted 0x80]    │
  │                                       │  all subsequent datagrams encrypted
```

AUTH_INIT and AUTH_CHALLENGE are sent **unencrypted** (no session key exists yet). The OK response is the first encrypted datagram, confirming both sides derived the same session key.

## Session lifecycle

- **Creation:** successful AUTH_PROOF → session entry with 16-byte random session_id
- **Usage:** every subsequent datagram includes the session_id in cleartext, encrypted payload inside
- **Idle timeout:** configurable via `--idle-timeout` (default 300 seconds). No activity = session evicted.
- **Termination:** key removed from authorized_keys + RELOAD → session terminated immediately

## Encryption

Each datagram is encrypted with **ChaCha20-Poly1305** using a unique 12-byte nonce. The session key is derived via X25519 key agreement during the handshake.

Random noise (64–1024 bytes from `getrandom`) is appended inside the encrypted block before the auth tag. This makes every datagram a unique, unpredictable size.

## Local clients

UDS clients are not authenticated via the handshake. Access control is handled by Unix file permissions on the socket:

```bash
chmod 0660 /var/run/tric/server.sock
chown tric:tric /var/run/tric/server.sock
```

Only users in the `tric` group can connect. This is simpler and faster than cryptographic auth for same-machine communication.

## Error responses

| Opcode | Meaning |
|--------|---------|
| `0xA5` | Authentication required — client sent a data opcode without a session |
| `0xA6` | Authentication failed — invalid signature or unknown public key |
| `0xA7` | Auth not configured — server started with `--no-auth` |

For decryption failures: **silent drop, no response.** Responding to a datagram that failed decryption would be an oracle.

## Next

- [Performance](10-performance.md) — benchmark methodology and tuning
- [Wire Protocol](04-wire-protocol.md) — datagram format details
