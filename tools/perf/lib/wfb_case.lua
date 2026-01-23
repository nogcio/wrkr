local common = require("lib.wfb_common")

local M = {}

local countries = { "US", "DE", "FR", "JP" }
local categories = { "Electronics", "Books", "Clothing", "Home" }

local OrderStatus = {
  COMPLETED = 1,
  PENDING = 2,
  FAILED = 3,
}

local statuses = {
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.PENDING,
  OrderStatus.FAILED,
  OrderStatus.PENDING,
}

local function init_zero_map(keys)
  local out = {}
  for _, k in ipairs(keys) do
    out[k] = 0
  end
  return out
end

-- Fast deterministic pseudo-random integer in [min, max].
-- Keeps the workload stable across tools/runs without math.randomseed.
local function prng_int(seed, min, max)
  local x = (seed * 1103515245 + 12345) % 2147483647
  local span = (max - min) + 1
  return min + (x % span)
end

-- Returns a stable test case resembling tools/perf/wfb_grpc_aggregate.lua.
-- Fields:
-- - client_id (string)
-- - orders (array)
-- - expected_processed (number)
-- - expected_results (map country->amount)
-- - expected_category_stats (map category->qty)
function M.generate_case(case_id)
  local num_orders = 100

  local orders = {}
  local expected_processed = 0
  local expected_results = init_zero_map(countries)
  local expected_category_stats = init_zero_map(categories)

  local client_id = "client-" .. tostring(case_id)

  for i = 0, num_orders - 1 do
    local status = statuses[(i % #statuses) + 1]
    local country = countries[(i % #countries) + 1]

    local items = {}
    local order_amount = 0

    for j = 0, 2 do
      local seed = (case_id * 100000) + (i * 10) + j
      local price = prng_int(seed, 1000, 10000)
      local quantity = prng_int(seed + 7, 1, 5)
      local category = categories[((i + j) % #categories) + 1]

      order_amount = order_amount + (price * quantity)
      table.insert(items, {
        quantity = quantity,
        category = category,
        price_cents = price,
      })

      if status == OrderStatus.COMPLETED then
        expected_category_stats[category] = expected_category_stats[category] + quantity
      end
    end

    table.insert(orders, {
      id = tostring(i + 1),
      status = status,
      country = country,
      items = items,
    })

    if status == OrderStatus.COMPLETED then
      expected_processed = expected_processed + 1
      expected_results[country] = expected_results[country] + order_amount
    end
  end

  return {
    client_id = client_id,
    orders = orders,
    expected_processed = expected_processed,
    expected_results = expected_results,
    expected_category_stats = expected_category_stats,
  }
end

function M.to_num(v)
  return common.to_num(v)
end

function M.totals_match(actual_tbl, expected_tbl)
  return common.totals_match(actual_tbl, expected_tbl)
end

return M
