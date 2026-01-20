options = {
  vus = 1,
  iterations = 1,
}

function Default(_data)
  local env = require("wrkr/env")
  local http = require("wrkr/http")
  local group = require("wrkr/group")
  local metrics = require("wrkr/metrics")

  group.group("g_http", function()
    local res = http.get(env.BASE_URL .. "/plaintext", {
      name = "GET /plaintext",
      tags = { scenario = "main" },
    })
    if res.status ~= 200 then
      error("unexpected status: " .. tostring(res.status))
    end
  end)

  group.group("g_http_override", function()
    local res = http.get(env.BASE_URL .. "/plaintext", {
      name = "GET /plaintext override",
      tags = { scenario = "main", group = "manual" },
    })
    if res.status ~= 200 then
      error("unexpected status: " .. tostring(res.status))
    end
  end)

  local counter = metrics.Counter("custom_counter")

  group.group("g_metric", function()
    counter:add(1, { k = "v" })

    local trend = metrics.Trend("custom_trend_tagged")
    trend:add(12.5, { scenario = "main" })
  end)
end
