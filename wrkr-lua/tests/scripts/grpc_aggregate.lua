options = { iterations = 1 }

local grpc = require("wrkr/grpc")
local check = require("wrkr/check")
local env = require("wrkr/env")

local client = grpc.Client.new()
client:load({ "protos" }, "protos/analytics.proto")

local connected = false

function Default()
  if not connected then
    local ok, err = client:connect(env.GRPC_TARGET, { timeout = "2s" })
    if not ok then error(err) end
    connected = true
  end

  local req = {
    orders = {
      {
        id = "o1",
        status = 1, -- COMPLETED
        country = "IE",
        items = {
          { quantity = 2, category = "books", price_cents = 100 },
          { quantity = 1, category = "games", price_cents = 500 },
        },
      },
      {
        id = "o2",
        status = 2, -- PENDING
        country = "US",
        items = {
          { quantity = 5, category = "books", price_cents = 100 },
        },
      },
    },
  }

  local res = client:invoke(
    "AnalyticsService/AggregateOrders",
    req,
    { name = "AggregateOrders", metadata = { ["x-client-id"] = "abc" } }
  )

  check(res, {
    ["ok"] = function(r) return r.ok == true end,
    ["echoed_client_id"] = function(r) return r.response.echoed_client_id == "abc" end,
    ["processed_orders"] = function(r) return r.response.processed_orders == 1 end,
    ["amount_by_country"] = function(r)
      return r.response.amount_by_country.IE == 700
    end,
    ["quantity_by_category"] = function(r)
      return r.response.quantity_by_category.books == 2
        and r.response.quantity_by_category.games == 1
    end,
  })
end
