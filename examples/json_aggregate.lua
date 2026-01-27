-- A port of web-framework-benchmark's wrkr_json_aggregate.lua scenario.
--
-- Expects an endpoint:
--   POST /json/aggregate
-- with body: JSON array of orders
-- and response JSON object:
--   { processedOrders = number, results = { [country]=amount }, categoryStats = { [category]=qty } }

Options = { vus = 50, duration = "10s" }

local http = require("wrkr/http")
local check = require("wrkr/check")
local json = require("wrkr/json")
local vu = require("wrkr/vu")
local ex = require("lib.example")
local checks = require("lib.checks")
local Pool = require("lib.pool")
local orders = require("lib.synthetic_orders")

local pool = Pool.new({
  size = 200,
  generate = function()
    return orders.generate_aggregate_case()
  end,
})

function Default()
  local base = ex.base_url()

  pool:ensure_initialized(vu.id())

  local data = pool:next()
  local res = http.post(base .. "/json/aggregate", data.orders, {
    headers = {
      accept = "application/json",
    },
  })

  local ctx = checks.json_ctx(res, json)
  ctx.expected = data

  local cs = {}
  cs["status is 200"] = checks.status_is(200)
  cs["body is valid json"] = checks.json_decode_ok()
  cs["body is json object"] = checks.json_body_is_object()
  cs["processedOrders matches"] = checks.json_field_equals("processedOrders", function(c)
    return c.expected.expected_processed
  end)
  cs["results match"] = checks.json_totals_match("results", function(c)
    return c.expected.expected_results
  end)
  cs["categoryStats match"] = checks.json_totals_match("categoryStats", function(c)
    return c.expected.expected_category_stats
  end)

  check(ctx, cs)
end

