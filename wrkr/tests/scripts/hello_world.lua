Options = { vus = 2 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local base = env.BASE_URL
  if base == nil then
    error("BASE_URL is required")
  end

  local res = http.get(base .. "/hello")
  check(res, {
    ["status is 200"] = function(r) return r.status == 200 end,
    ["body is Hello World"] = function(r) return r.body == "Hello World!" end,
  })
end
