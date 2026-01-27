Options = { iterations = 1 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local base = env.BASE_URL
  if base == nil then
    error("BASE_URL is required")
  end

  local body_tbl = { 1, 2, 3 }

  local res = http.post(base .. "/echo", body_tbl, {
    headers = {
      ["x-test"] = "1",
    },
  })

  check(res, {
    ["status is 200"] = function(r) return r.status == 200 end,
    ["echo body is json"] = function(r) return r.body == "[1,2,3]" end,
  })
end
