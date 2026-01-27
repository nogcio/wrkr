local grpc = require("wrkr/grpc")
local check = require("wrkr/check")
local env = require("wrkr/env")

local target = env.BASE_URL
if target == nil then
  error("BASE_URL is required")
end

local client = grpc.Client.new()
local ok = pcall(function()
  client:load({ "tools/perf/protos" }, "tools/perf/protos/echo.proto")
end)
if not ok then
  client:load({ "protos" }, "protos/echo.proto")
end


local req = { message = "ping" }
local invoke_opts = { name = "Echo" }
local checks = {
  ["ok"] = function(r)
    return r.ok == true
  end,
  ["echo"] = function(r)
    return r.response ~= nil and r.response.message == "ping"
  end,
}

local connected = false

function Default()
  if not connected then
    local connect_ok, err = client:connect(target, { timeout = "3s" })
    if not connect_ok then
      error("gRPC connect failed: " .. tostring(err))
    end
    connected = true
  end
  local res = client:invoke("wrkr.test.EchoService/Echo", req, invoke_opts)
  check(res, checks)
end
