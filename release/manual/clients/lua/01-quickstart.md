# Lua Bridge: Quickstart

The TRIC+ Lua client is a loadable C module (`tric.so` / `tric.dylib`) registered with the Lua interpreter via `require('tric')`. It returns a module table with `connect`, which yields a userdata with metatable methods (`conn:read`, `conn:write`, etc.). Automatic garbage collection via the `__gc` metamethod releases the connection when Lua reaps the userdata.

## Requirements

- **Lua 5.4+** (`brew install lua` on macOS; packaged on most Linux distributions)
- A C compiler (cc / clang / gcc) with Lua development headers
- A running TRIC+ server reachable via a Unix-domain socket

## Build

From the repo root:

```bash
bridges/lua/build.sh
```

The script auto-detects the Lua install prefix (default `/opt/homebrew/opt/lua` on macOS; override with `LUA_PREFIX`). Output: `bridges/lua/tric.dylib` (macOS) or `tric.so` (Linux/BSD). The file name is `tric.{ext}` so `require('tric')` finds it via `package.cpath`.

## Load and connect

```lua
-- Put bridges/lua/ on package.cpath, then:
local tric = require('tric')

local conn = tric.connect('/var/run/tric/server.sock')
if not conn:valid() then
    error('connect failed')
end

-- ... use the connection ...

-- `__gc` runs automatically when `conn` becomes unreachable.
-- For deterministic cleanup, set `conn = nil` and call `collectgarbage()`.
```

`tric.connect` binds a temporary socket at `/tmp/tric-c-{pid}.sock` and connects to the server. The returned userdata carries the connection; Lua's GC closes it via the `__gc` metamethod when the userdata is collected.

## Primitives

### Write and read

```lua
conn:write('user:42', 'alice')

local value = conn:read('user:42')
if value then
    print(value)  -- 'alice'
end
```

`conn:read` returns the value as a Lua string, which is 8-bit clean and holds any bytes including nulls. It returns `nil` if the key is absent.

### Delete

```lua
conn:del('user:42')
```

Raises a Lua error on socket failure (catch with `pcall`). Deleting a missing key succeeds silently.

### Compare-and-delete

```lua
local matched = conn:cad('job:1', 'pending')
-- matched == true:  value was 'pending', key is now deleted
-- matched == false: value was something else, key is untouched
```

Atomic. Returns boolean. Raises on communication failure.

### TTL

```lua
conn:write('session:abc', 'token')
conn:ttl('session:abc', 3600000)
```

Duration in milliseconds as Lua integer. Missing key is a silent no-op.

### Prefix scan

```lua
local pairs_result = conn:scan('user:')
for _, pair in ipairs(pairs_result) do
    print(pair.key, '=', pair.value)
end
```

Returns an array of tables, each with `key` and `value` fields (both Lua strings, 8-bit clean).

## API

| Method | Signature | Purpose |
|--------|-----------|---------|
| `tric.connect` | `(socketPath) -> Connection` | Open a connection |
| `conn:valid` | `() -> boolean` | Check if the connection is live |
| `conn:read` | `(key) -> string|nil` | Fetch a value; `nil` if absent |
| `conn:write` | `(key, value)` | Store a value |
| `conn:del` | `(key)` | Remove a key |
| `conn:cad` | `(key, expected) -> boolean` | Atomic compare-and-delete |
| `conn:ttl` | `(key, durationMs)` | Set expiry on an existing key |
| `conn:scan` | `(prefix) -> {{key, value}, …}` | Fetch all pairs by key prefix |

## Error handling

Communication failures raise Lua errors; catch with:

```lua
local ok, err = pcall(function() conn:write(k, v) end)
if not ok then
    print('write failed: ' .. err)
end
```

Absent values → `nil`. `cad` mismatch → `false`.

## Test

Tests live at `bridges/lua/tests/bridge_test.lua` (plain Lua `assert`-style, no framework dependency). Start a scratch server, run the test:

```bash
cargo build --release

mkdir -p /tmp/tric-lua-test
TRIC_SOCKET_DIR=/tmp/tric-lua-test \
TRIC_BASE_DIR=/tmp/tric-lua-test/data \
TRIC_INSTANCE=luatest TRIC_SLOT=0 \
./target/release/tric server &
SERVER_PID=$!
sleep 2

bridges/lua/build.sh   # if not already built
TRIC_SOCKET=/tmp/tric-lua-test/server.sock \
lua bridges/lua/tests/bridge_test.lua

kill $SERVER_PID
rm -rf /tmp/tric-lua-test
```

Output: `14 passed, 0 failed`.

## Next

- [C Bridge Quickstart](../c/01-quickstart.md): the underlying C layer that the Lua module links against.
- [Tcl Bridge Quickstart](../tcl/01-quickstart.md) : sibling loadable-C-extension pattern
- [Client Overview](../00-overview.md) : the wire protocol from the client perspective, plus the minimum API surface every bridge must provide
- [Wire Protocol](../../server/04-wire-protocol.md) : the full opcode reference, including request and response formats for every primitive
