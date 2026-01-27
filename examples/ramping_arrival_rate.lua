-- Run: wrkr run examples/ramping_arrival_rate.lua --env BASE_URL=http://localhost:8080

Options = {
  scenarios = {
    main = {
      executor = "ramping-arrival-rate",
      startRate = 100,
      timeUnit = "1s",
      preAllocatedVUs = 200,
      maxVUs = 2000,
      stages = {
        { duration = "10s", target = 1400 },
        { duration = "5s", target = 1400 },
        { duration = "10s", target = 100 },
      },
      exec = "Default",
    },
  },
}

local http = require("wrkr/http")
local env = require("wrkr/env")

function Default()
  http.get(env.BASE_URL .. "/plaintext")
end
