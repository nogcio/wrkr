options = { vus = 100, duration = "10s" }

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")
local checks = require("lib.checks")

function Default()
  local base = env.BASE_URL

  local res = http.get(base .. "/plaintext")
  check(res, {
    ["status is 200"] = checks.status_is(200),
    ["body is Hello World"] = checks.body_equals("Hello, World!"),
  })
end
