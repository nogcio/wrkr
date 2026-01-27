Options = { iterations = 1 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local base = env.BASE_URL
  local res = http.get(base .. "/slow", { timeout = '1ms' })
  check(res, {
    ["timed out"] = function(r) return r.status == 0 and r.error ~= nil end,
  })
end
