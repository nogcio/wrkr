# wrkr/grpc

gRPC client helpers.

```lua
local grpc = require("wrkr/grpc")
```

## Client

### `grpc.Client.new() -> client`

Creates a client object.

### `client:load(paths, file) -> true | (nil, err)`

Compiles a protobuf schema.

- `paths`: array of include paths (strings)
- `file`: path to the `.proto` file

Paths are resolved relative to the script directory unless absolute.

### `client:connect(target, opts?) -> true | (nil, err)`

Connects to `target`.

Options:

- `timeout`: duration string (e.g. `"2s"`)
- `tls`: table:
  - `server_name`: string
  - `insecure_skip_verify`: boolean
  - `ca`: string (PEM bytes)
  - `cert`: string (PEM bytes)
  - `key`: string (PEM bytes)

### `client:invoke(full_method, req, opts?) -> res`

Performs a unary request.

- `full_method`: string like `"pkg.Service/Method"`
- `req`: Lua value (converted into protobuf message)
- `opts`:
  - `name`: string metric name
  - `timeout`: duration string
  - `metadata`: table<string, string|string[]>
  - `tags`: table<string, string|number|boolean>

Returns a response table:

- `ok`: boolean
- `status`: integer? (0..16), `nil` on transport error
- `message`: string?
- `error`: string?
- `error_kind`: string?
- `headers`: table<string, string>?
- `trailers`: table<string, string>?
- `response`: table?

Notes:

- On runtime errors (not loaded / not connected / transport), `invoke` returns a response table with `ok=false` and does not throw.
- If called inside a [wrkr/group](group.md) group, a `group` tag is added unless you already set one.

## Example

See [grpc_aggregate.lua](../../examples/grpc_aggregate.md).
