Options = { iterations = 1 }

local grpc = require("wrkr/grpc")
local check = require("wrkr/check")
local env = require("wrkr/env")

local client = grpc.Client.new()
client:load({ "protos" }, "protos/echo.proto")

local connected = false

function Default()
  if not connected then
    local ok, err = client:connect(env.BASE_URL, { timeout = "2s" })
    if not ok then error(err) end
    connected = true
  end

  local res = client:invoke(
    "wrkr.test.EchoService/Echo",
    { message = "ping" },
    { name = "Echo", metadata = { ["x-test"] = "1" } }
  )

  check(res, {
    ["ok"] = function(r) return r.ok == true end,
    ["echo"] = function(r) return r.response.message == "ping" end,
  })
end
