local grpc = require("wrkr/grpc")
local check = require("wrkr/check")
local vu = require("wrkr/vu")
local uuid = require("wrkr/uuid")
local env = require("wrkr/env")

local Pool = require("lib.pool")
local wfb = require("lib.wfb")

Options = wfb.ramping_vus_options(wfb.max_vus(50), wfb.duration("10s"))

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

local client

local function init_zero_map(keys)
  local out = {}
  for _, k in ipairs(keys) do
    out[k] = 0
  end
  return out
end

local function generate_case()
  local num_orders = 100

  local orders = {}
  local expected_processed = 0
  local expected_results = init_zero_map(countries)
  local expected_category_stats = init_zero_map(categories)

  local client_id = uuid.v4()

  for i = 0, num_orders - 1 do
    local status = statuses[(i % #statuses) + 1]
    local country = countries[(i % #countries) + 1]

    local items = {}
    local order_amount = 0

    for j = 0, 2 do
      local price = math.random(1000, 10000)
      local quantity = math.random(1, 5)
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

  local req_bytes, err = client:encode(
    "AnalyticsService/AggregateOrders",
    { orders = orders }
  )
  if req_bytes == nil then
    error(err or "failed to encode request")
  end

  return {
    client_id = client_id,
    orders = orders,
    req_bytes = req_bytes,
    expected_processed = expected_processed,
    expected_results = expected_results,
    expected_category_stats = expected_category_stats,
  }
end

local pool = Pool.new({
  size = 50,
  generate = generate_case,
})

local target = env.BASE_URL
if target == nil then
  error("BASE_URL is required")
end

client = grpc.Client.new()
local ok_load = pcall(function()
  client:load({ "tools/perf/protos" }, "tools/perf/protos/analytics.proto")
end)
if not ok_load then
  client:load({ "protos" }, "protos/analytics.proto")
end

local connected = false

function Default()
  if not connected then
    local ok_connect, err = client:connect(target, { timeout = "2s" })
    if not ok_connect then
      error(err or "failed to connect")
    end
    connected = true
  end

  pool:ensure_initialized(vu.id())
  local data = pool:next()

  local res = client:invoke(
    "AnalyticsService/AggregateOrders",
    data.req_bytes,
    {
      name = "gRPC AggregateOrders (wfb)",
      metadata = {
        ["x-client-id"] = data.client_id,
      }
    }
  )

  local ctx = {
    res = res,
    expected = data,
  }

  check(ctx, {
    ["grpc ok"] = function(c)
      return type(c.res) == "table" and c.res.ok == true and c.res.status == 0
    end,
    ["has response table"] = function(c)
      return type(c.res) == "table" and type(c.res.response) == "table"
    end,
    ["echoed_client_id matches"] = function(c)
      if type(c.res) ~= "table" or type(c.res.response) ~= "table" then
        return false
      end
      return c.res.response.echoed_client_id == c.expected.client_id
    end,
    ["processed_orders matches"] = function(c)
      if type(c.res) ~= "table" or type(c.res.response) ~= "table" then
        return false
      end
      return wfb.to_num(c.res.response.processed_orders) == c.expected.expected_processed
    end,
    ["amount_by_country matches"] = function(c)
      if type(c.res) ~= "table" or type(c.res.response) ~= "table" then
        return false
      end
      return wfb.totals_match(c.res.response.amount_by_country, c.expected.expected_results)
    end,
    ["quantity_by_category matches"] = function(c)
      if type(c.res) ~= "table" or type(c.res.response) ~= "table" then
        return false
      end
      return wfb.totals_match(c.res.response.quantity_by_category, c.expected.expected_category_stats)
    end,
  })
end
