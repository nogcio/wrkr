-- Synthetic data generator for the gRPC analytics aggregate benchmark.
--
-- Matches `examples/protos/analytics.proto`.

local M = {}

local uuid = require("wrkr/uuid")

M.defaults = {
  countries = { "US", "DE", "FR", "JP" },
  categories = { "Electronics", "Books", "Clothing", "Home" },

  num_orders = 100,
  items_per_order = 3,

  -- Price in cents.
  price_min = 1000,
  price_max = 10000,

  quantity_min = 1,
  quantity_max = 5,

  -- ~70% completed, to match WFB benchmark intent.
  statuses = {
    1, 1, 1, 1, 1, 1, 1, -- COMPLETED
    2, -- PENDING
    3, -- FAILED
    2, -- PENDING
  },
}

local function get(t, k, default)
  if t ~= nil and t[k] ~= nil then
    return t[k]
  end
  return default
end

local function init_zero_map(keys)
  local out = {}
  for _, k in ipairs(keys) do
    out[k] = 0
  end
  return out
end

local function new_client_id()
  return uuid.v4()
end

-- Generates one case:
--  - orders: array<Order>
--  - client_id: string
--  - expected_processed: number
--  - expected_results: map[country]amount_cents (number)
--  - expected_category_stats: map[category]qty (number)
function M.generate_aggregate_case(opts)
  opts = opts or {}

  local countries = get(opts, "countries", M.defaults.countries)
  local categories = get(opts, "categories", M.defaults.categories)
  local statuses = get(opts, "statuses", M.defaults.statuses)

  local num_orders = get(opts, "num_orders", M.defaults.num_orders)
  local items_per_order = get(opts, "items_per_order", M.defaults.items_per_order)

  local price_min = get(opts, "price_min", M.defaults.price_min)
  local price_max = get(opts, "price_max", M.defaults.price_max)

  local quantity_min = get(opts, "quantity_min", M.defaults.quantity_min)
  local quantity_max = get(opts, "quantity_max", M.defaults.quantity_max)

  local orders = {}
  local expected_processed = 0
  local expected_results = init_zero_map(countries)
  local expected_category_stats = init_zero_map(categories)

  local client_id = new_client_id()

  for i = 0, num_orders - 1 do
    local status = statuses[(i % #statuses) + 1]
    local country = countries[(i % #countries) + 1]

    local items = {}
    local order_amount = 0

    for j = 0, items_per_order - 1 do
      local price = math.random(price_min, price_max)
      local quantity = math.random(quantity_min, quantity_max)
      local category = categories[((i + j) % #categories) + 1]

      order_amount = order_amount + (price * quantity)
      table.insert(items, {
        quantity = quantity,
        category = category,
        price_cents = price,
      })

      if status == 1 then
        expected_category_stats[category] = expected_category_stats[category] + quantity
      end
    end

    table.insert(orders, {
      id = tostring(i + 1),
      status = status,
      country = country,
      items = items,
    })

    if status == 1 then
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

return M
