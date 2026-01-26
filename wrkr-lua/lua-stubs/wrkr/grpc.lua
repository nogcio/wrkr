---@meta

---@class wrkr.grpc
local M = {}

---@class wrkr.grpc.ClientModule
local ClientModule = {}

---@class wrkr.grpc.TlsOptions
---@field ca string? PEM bytes
---@field cert string? PEM bytes
---@field key string? PEM bytes
---@field server_name string? SNI / domain name
---@field insecure_skip_verify boolean?

---@class wrkr.grpc.NewOptions
---@field pool_size integer? Number of TCP connections in the shared pool (default = clamp(floor(max_vus / 8), 16, 64))

---@class wrkr.grpc.ConnectOptions
---@field timeout string? e.g. "3s"
---@field tls wrkr.grpc.TlsOptions?

---@class wrkr.grpc.InvokeOptions
---@field timeout string? e.g. "1s"
---@field metadata table<string, string|string[]>?
---@field tags table<string, string|number|boolean>?
---@field int64 'integer'|'string'? How to represent int64 values in the response (default: 'integer').

---@class wrkr.grpc.UnaryResponse
---@field ok boolean
---@field status integer? gRPC status code (0..16). nil for transport error.
---@field message string?
---@field error string?
---@field error_kind string?
---@field response table?

---@class wrkr.grpc.Client
local Client = {}

---@return wrkr.grpc.Client
---@param opts wrkr.grpc.NewOptions?
function ClientModule.new(opts)
	return Client
end

---@param paths string[]
---@param file string
---@return boolean|nil, string? err
function Client:load(paths, file)
	return true
end

---@param target string
---@param opts wrkr.grpc.ConnectOptions?
---@return boolean|nil, string? err
function Client:connect(target, opts)
	return true
end

---@param full_method string @"pkg.Service/Method"
---@param req any|string Either a request table/object, or protobuf-encoded request bytes (Lua string).
---@param opts wrkr.grpc.InvokeOptions?
---@return wrkr.grpc.UnaryResponse
function Client:invoke(full_method, req, opts)
	return { ok = true, status = 0, response = {} }
end

---@param full_method string @"pkg.Service/Method"
---@param req any
---@return string|nil, string? err Protobuf-encoded request bytes
function Client:encode(full_method, req)
	return ""
end

M.Client = ClientModule

return M
