-- Perf case: POST /analytics/aggregate with a wfb-style payload + response checks.
-- Stresses wrkr/http + wrkr/json encode/decode + wrkr/check.

options = { vus = 50 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local json = require("wrkr/json")
local env = require("wrkr/env")
local vu = require("wrkr/vu")

local Pool = require("lib.pool")
local wfb_case = require("lib.wfb_case")

local base = env.BASE_URL
if base == nil then
  error("BASE_URL is required")
end

local url = base .. "/analytics/aggregate"

local case_id = 0

local pool = Pool.new({
  size = 50,
  generate = function()
    case_id = case_id + 1
    return wfb_case.generate_case((vu.id() * 100000) + case_id)
  end,
  seed = function(_vu_id)
    -- no-op: wfb_case is deterministic
  end,
})

local request_opts = {
  headers = {
    ["content-type"] = "application/json",
    ["accept"] = "application/json",
    ["x-test"] = "1",
  },
}

function Default()
  pool:ensure_initialized(vu.id())
  local data = pool:next()

  -- Encode every iteration (to match k6/ wrk behavior and stress JSON encoding).
  local body = json.encode({
    client_id = data.client_id,
    orders = data.orders,
  })

  local res = http.post(url, body, request_opts)

  local decoded = nil
  if type(res) == "table" and type(res.body) == "string" then
    decoded = json.decode(res.body)
  end

  local ctx = { res = res, decoded = decoded, expected = data }

  check(ctx, {
    ["status is 200"] = function(c)
      return type(c.res) == "table" and c.res.status == 200
    end,
    ["has decoded table"] = function(c)
      return type(c.decoded) == "table"
    end,
    ["echoed_client_id matches"] = function(c)
      return type(c.decoded) == "table" and c.decoded.echoed_client_id == c.expected.client_id
    end,
    ["processed_orders matches"] = function(c)
      return type(c.decoded) == "table"
        and wfb_case.to_num(c.decoded.processed_orders) == c.expected.expected_processed
    end,
    ["amount_by_country matches"] = function(c)
      return type(c.decoded) == "table"
        and wfb_case.totals_match(c.decoded.amount_by_country, c.expected.expected_results)
    end,
    ["quantity_by_category matches"] = function(c)
      return type(c.decoded) == "table"
        and wfb_case.totals_match(c.decoded.quantity_by_category, c.expected.expected_category_stats)
    end,
  })
end
