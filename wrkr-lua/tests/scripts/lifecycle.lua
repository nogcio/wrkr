Options = {
  vus = 2,
  iterations = 4,
}

function Setup()
  local shared = require("wrkr/shared")
  shared.set("setup", { token = "abc" })
end

function Default()
  local shared = require("wrkr/shared")
  local data = shared.get("setup")
  if data == nil or data.token ~= "abc" then
    error("missing setup data")
  end

  local env = require("wrkr/env")
  local http = require("wrkr/http")
  local group = require("wrkr/group")
  local metrics = require("wrkr/metrics")

  local t = metrics.Trend("custom_trend")
  t:add(123.4)

  group.group("plaintext", function()
    local res = http.get(env.BASE_URL .. "/plaintext", {
      name = "GET /plaintext",
      tags = { scenario = "main" },
    })
    if res.status ~= 200 then
      error("unexpected status: " .. tostring(res.status))
    end
  end)
end

function Teardown()
  local shared = require("wrkr/shared")
  local data = shared.get("setup")
  if data == nil or data.token ~= "abc" then
    error("missing setup data in teardown")
  end
end

function HandleSummary(summary)
  return {
    ["summary.txt"] = "ok\n",
  }
end


