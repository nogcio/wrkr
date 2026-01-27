Options = { iterations = 1 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local base = env.BASE_URL
  local res = http.post(base .. "/echo", "ping", { headers = { ["x-test"] = "1" } })
  check(res, {
    ["status is 200"] = function(r) return r.status == 200 end,
    ["body echo"] = function(r) return r.body == "ping" end,
  })
end
