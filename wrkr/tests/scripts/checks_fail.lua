options = { vus = 1 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local base = env.BASE_URL
  if base == nil then
    error("BASE_URL is required")
  end

  local res = http.get(base .. "/hello")

  -- Intentionally fail.
  check(res, {
    ["status is 201"] = function(r) return r.status == 201 end,
  })
end
