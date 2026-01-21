local grpc = require("wrkr/grpc")
local check = require("wrkr/check")
local env = require("wrkr/env")

local target = env.GRPC_TARGET
if target == nil then
  error("GRPC_TARGET is required")
end

local client = grpc.Client.new()
local ok = pcall(function()
  client:load({ "tools/perf/protos" }, "tools/perf/protos/echo.proto")
end)
if not ok then
  client:load({ "protos" }, "protos/echo.proto")
end

local connected = false

function Default()
  if not connected then
    local ok, err = client:connect(target, { timeout = "2s" })
    if not ok then
      error(err)
    end
    connected = true
  end

  local res = client:invoke(
    "wrkr.test.EchoService/Echo",
    { message = "ping" },
    {
      name = "Echo",
    }
  )

  check(res, {
    ["ok"] = function(r)
      return r.ok == true
    end,
    ["echo"] = function(r)
      return r.response.message == "ping"
    end,
  })
end
