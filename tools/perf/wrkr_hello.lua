-- Minimal script for perf comparisons (keep overhead low).
-- Use CLI flags to control vus/duration.
Options = { vus = 50 }

local http = require("wrkr/http")
local env = require("wrkr/env")

local base = env.BASE_URL
if base == nil then
  error("BASE_URL is required")
end
local hello_url = base .. "/hello"

function Default()
  -- No checks here: we're benchmarking request loop overhead.
  http.get(hello_url)
end
