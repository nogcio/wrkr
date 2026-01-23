-- Run:
--   wrkr run examples/plaintext.lua --env BASE_URL=http://localhost:8080 --dashboard

options = {
  scenarios = {
    -- A steady baseline: constant concurrency for a fixed duration.
    steady = {
      executor = "constant-vus",
      vus = 25,
      duration = "20s",
      exec = "Plaintext",
    },

    -- Ramp VUs up and down over time.
    ramp_vus = {
      executor = "ramping-vus",
      startVUs = 0,
      stages = {
        { duration = "5s", target = 50 },
        { duration = "10s", target = 50 },
        { duration = "5s", target = 0 },
      },
      exec = "Plaintext",
    },

    -- Ramp an open-model arrival rate (iterations started per timeUnit).
    ramp_rate = {
      executor = "ramping-arrival-rate",
      startRate = 50,
      timeUnit = "1s",
      preAllocatedVUs = 50,
      maxVUs = 500,
      stages = {
        { duration = "5s", target = 200 },
        { duration = "5s", target = 200 },
        { duration = "5s", target = 50 },
      },
      exec = "Slow",
    },
  },
}

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")
local checks = require("lib.checks")

function Plaintext()
  local base = env.BASE_URL

  local res = http.get(base .. "/plaintext")
  check(res, {
    ["status is 200"] = checks.status_is(200),
    ["body is Hello World"] = checks.body_equals("Hello World!"),
  })
end

function Slow()
  local base = env.BASE_URL

  local res = http.get(base .. "/slow")
  check(res, {
    ["status is 200"] = checks.status_is(200),
    ["body is slow"] = checks.body_equals("slow"),
  })
end
