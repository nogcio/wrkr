-- Example showing lifecycle hooks.
--
-- Run:
--   wrkr run examples/lifecycle.lua --env BASE_URL=http://localhost:8080

Options = { vus = 5, duration = "5s" }

local env = require("wrkr/env")
local http = require("wrkr/http")
local shared = require("wrkr/shared")

function Setup()
  -- Runs once before scenarios (best-effort). Great place to fetch a token.
  shared.set("token", "abc")
end

function Default()
  -- Reads data created during Setup.
  local token = shared.get("token")
  if token == nil then
    error("missing token")
  end

  http.get(env.BASE_URL .. "/plaintext", {
    headers = { ["x-token"] = token },
    name = "GET /plaintext",
  })
end

function Teardown()
  -- Runs once after scenarios (best-effort).
  shared.delete("token")
end

function HandleSummary(summary)
  -- Return a table of outputs; keys other than stdout/stderr are written as files
  -- relative to the current working directory.
  return {
    stdout = string.format("requests_total=%d\n", summary.requests_total),
    ["summary.json"] = require("wrkr/json").encode(summary) .. "\n",
  }
end
