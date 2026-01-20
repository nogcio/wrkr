options = { vus = 100, duration = "10s" }

local http = require("wrkr/http")
local check = require("wrkr/check")
local ex = require("lib.example")
local checks = require("lib.checks")

function Default()
  local base = ex.base_url()

  local res = http.get(base .. "/plaintext")
  check(res, {
    ["status is 200"] = checks.status_is(200),
    ["body is Hello World"] = checks.body_equals("Hello, World!"),
  })
end
