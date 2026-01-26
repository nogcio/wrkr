-- A port of web-framework-benchmark's wrkr_grpc_aggregate.lua scenario.
--
-- Expects a gRPC service matching `examples/protos/analytics.proto`:
--   service AnalyticsService { rpc AggregateOrders(AnalyticsRequest) returns (AggregateResult); }
--
-- Required env vars:
--   GRPC_TARGET (e.g. "http://127.0.0.1:50051")

options = { vus = 50, duration = "10s" }

local grpc = require("wrkr/grpc")
local check = require("wrkr/check")
local env = require("wrkr/env")
local vu = require("wrkr/vu")

local Pool = require("lib.pool")
local gen = require("lib.synthetic_analytics")
local gchecks = require("lib.grpc_checks")
local ex = require("lib.example")

local base = ex.base_url()

local client = grpc.Client.new()
client:load({ "protos" }, "protos/analytics.proto")

local connected = false

local function ensure_connected()
  if connected then
    return
  end

  local ok, err = client:connect(base, { timeout = "2s" })
  if not ok then
    error(err)
  end

  connected = true
end

local pool = Pool.new({
  size = 50,
  generate = function()
    local data = gen.generate_aggregate_case()

    local req_bytes, enc_err = client:encode(
      "AnalyticsService/AggregateOrders",
      { orders = data.orders }
    )
    if req_bytes == nil then
      error("failed to encode AnalyticsRequest: " .. tostring(enc_err))
    end

    data.req_bytes = req_bytes
    return data
  end,
})

function Default()
  ensure_connected()
  pool:ensure_initialized(vu.id())
  local data = pool:next()

  local res = client:invoke(
    "AnalyticsService/AggregateOrders",
    data.req_bytes,
    {
      name = "gRPC AggregateOrders",
      timeout = "10s",
      metadata = {
        ["x-client-id"] = data.client_id,
      },
      tags = {
        workload = "grpc_aggregate",
      },
    }
  )

  local ctx = { expected = data }
  gchecks.with_res(ctx, res)

  check(ctx, {
    ["grpc ok"] = gchecks.grpc_ok(),
    ["has response"] = gchecks.grpc_has_response_table(),
    ["echoed_client_id matches"] = gchecks.grpc_field_equals("echoed_client_id", function(c)
      return c.expected.client_id
    end),
    ["processed_orders matches"] = gchecks.grpc_int_field_equals("processed_orders", function(c)
      return c.expected.expected_processed
    end),
    ["amount_by_country matches"] = gchecks.grpc_totals_match("amount_by_country", function(c)
      return c.expected.expected_results
    end),
    ["quantity_by_category matches"] = gchecks.grpc_totals_match("quantity_by_category", function(c)
      return c.expected.expected_category_stats
    end),
  })
end
