options = { iterations = 1 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")
local fs = require("wrkr/fs")

local checks = require("lib.checks")

function Default()
  local base = env.BASE_URL
  local body = fs.read_file("data/payload.txt")

  local res = http.post(base .. "/echo", body)
  check(res, {
    ["status is 200"] = function(r) return checks.status_is(r, 200) end,
    ["body matches payload"] = function(r) return checks.body_is(r, body) end,
  })
end
