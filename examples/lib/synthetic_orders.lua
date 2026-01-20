-- Synthetic orders generator for benchmarks.
--
-- Produces data shaped for JSON aggregation workloads.
-- Designed to be reused across multiple scripts.

local M = {}

M.defaults = {
  countries = { "US", "DE", "FR", "UK", "JP" },
  statuses = { "completed", "pending", "failed" },
  categories = { "Electronics", "Books", "Clothing", "Home" },

  num_orders = 100,
  items_per_order = 3,

  -- price in cents
  price_min = 1000,
  price_max = 10000,

  quantity_min = 1,
  quantity_max = 5,
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

-- Generates a single aggregate test case:
--  - orders: array
--  - expected_processed: number
--  - expected_results: map[country]amount
--  - expected_category_stats: map[category]qty
function M.generate_aggregate_case(opts)
  opts = opts or {}

  local countries = get(opts, "countries", M.defaults.countries)
  local statuses = get(opts, "statuses", M.defaults.statuses)
  local categories = get(opts, "categories", M.defaults.categories)

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

  for i = 0, num_orders - 1 do
    local status = statuses[(i % #statuses) + 1]
    local country = countries[(i % #countries) + 1]

    local items = {}
    local total_amount = 0

    for j = 0, items_per_order - 1 do
      local price = math.random(price_min, price_max)
      local quantity = math.random(quantity_min, quantity_max)
      local category = categories[((i + j) % #categories) + 1]

      total_amount = total_amount + (price * quantity)
      table.insert(items, {
        quantity = quantity,
        price = price,
        category = category,
      })

      if status == "completed" then
        expected_category_stats[category] = expected_category_stats[category] + quantity
      end
    end

    table.insert(orders, {
      id = tostring(i + 1),
      status = status,
      amount = total_amount,
      country = country,
      items = items,
    })

    if status == "completed" then
      expected_processed = expected_processed + 1
      expected_results[country] = expected_results[country] + total_amount
    end
  end

  return {
    orders = orders,
    expected_processed = expected_processed,
    expected_results = expected_results,
    expected_category_stats = expected_category_stats,
  }
end

return M
