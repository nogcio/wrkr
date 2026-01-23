-- wrk script: POST /analytics/aggregate with a wfb-style JSON payload + response validation.
-- URL passed to wrk should be the BASE_URL (e.g. http://127.0.0.1:1234)

-- Ensure we can require sibling modules from repo root.
package.path = "tools/perf/?.lua;tools/perf/?/init.lua;" .. package.path

local Pool = require("lib.pool")
local wfb_case = require("lib.wfb_case")

wrk.method = "POST"
wrk.path = "/analytics/aggregate"
wrk.headers["Content-Type"] = "application/json"
wrk.headers["Accept"] = "application/json"
wrk.headers["x-test"] = "1"

local errors = 0
local case_id = 0

local pool = Pool.new({
  size = 50,
  generate = function()
    case_id = case_id + 1
    return wfb_case.generate_case(case_id)
  end,
  seed = function(_tid)
    -- no-op: deterministic case generation
  end,
})

local function json_escape(s)
  -- client_id is simple, but keep this safe.
  return (tostring(s):gsub('\\', '\\\\'):gsub('"', '\\"'))
end

local function encode_request(data)
  -- Encode a fixed-shape JSON payload; avoids requiring a JSON lib.
  local parts = {}
  parts[#parts + 1] = '{"client_id":"'
  parts[#parts + 1] = json_escape(data.client_id)
  parts[#parts + 1] = '","orders":['

  for oi = 1, #data.orders do
    local o = data.orders[oi]
    if oi > 1 then
      parts[#parts + 1] = ','
    end

    parts[#parts + 1] = '{"id":"'
    parts[#parts + 1] = json_escape(o.id)
    parts[#parts + 1] = '","status":'
    parts[#parts + 1] = tostring(o.status)
    parts[#parts + 1] = ',"country":"'
    parts[#parts + 1] = json_escape(o.country)
    parts[#parts + 1] = '","items":['

    for ii = 1, #o.items do
      local it = o.items[ii]
      if ii > 1 then
        parts[#parts + 1] = ','
      end
      parts[#parts + 1] = '{"quantity":'
      parts[#parts + 1] = tostring(it.quantity)
      parts[#parts + 1] = ',"category":"'
      parts[#parts + 1] = json_escape(it.category)
      parts[#parts + 1] = '","price_cents":'
      parts[#parts + 1] = tostring(it.price_cents)
      parts[#parts + 1] = '}'
    end

    parts[#parts + 1] = ']}'
  end

  parts[#parts + 1] = ']}'
  return table.concat(parts)
end

local function match_number(body, key)
  local pat = '"' .. key .. '":(-?%d+)'
  local m = string.match(body, pat)
  if m == nil then
    return nil
  end
  return tonumber(m)
end

local function match_string(body, key)
  local pat = '"' .. key .. '":"([^"]*)"'
  return string.match(body, pat)
end

local function validate_totals(body, obj_key, expected)
  if expected == nil then
    return false
  end

  -- Ensure the object exists.
  if not string.find(body, '"' .. obj_key .. '":{', 1, true) then
    return false
  end

  for k, v in pairs(expected) do
    local m = match_number(body, k)
    if m == nil or m ~= v then
      return false
    end
  end

  return true
end

request = function()
  pool:ensure_initialized(0)
  local data = pool:next()

  -- Stash expected values for response() via globals.
  wrk._expected = data

  local payload = encode_request(data)
  return wrk.format(nil, nil, nil, payload)
end

response = function(status, _headers, body)
  if status ~= 200 then
    errors = errors + 1
    return
  end

  if type(body) ~= "string" then
    errors = errors + 1
    return
  end

  local expected = wrk._expected
  if expected == nil then
    errors = errors + 1
    return
  end

  local echoed = match_string(body, "echoed_client_id")
  if echoed ~= expected.client_id then
    errors = errors + 1
    return
  end

  local processed = match_number(body, "processed_orders")
  if processed == nil or processed ~= expected.expected_processed then
    errors = errors + 1
    return
  end

  if not validate_totals(body, "amount_by_country", expected.expected_results) then
    errors = errors + 1
    return
  end

  if not validate_totals(body, "quantity_by_category", expected.expected_category_stats) then
    errors = errors + 1
    return
  end
end

done = function(_summary, _latency, _requests)
  io.write(string.format("lua_errors: %d\n", errors))
end
