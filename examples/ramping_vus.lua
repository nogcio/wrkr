-- Run: wrkr run examples/ramping_vus.lua --env BASE_URL=http://localhost:8080

Options = {
  scenarios = {
    main = {
      executor = "ramping-vus",
      startVUs = 0,
      stages = {
        { duration = "5s", target = 50 },
        { duration = "5s", target = 0 },
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
