-- Heavier Lua perf case: POST JSON + validate response.
-- Stresses wrkr/json encode+decode + wrkr/check.

Options = { vus = 50 }

local http = require("wrkr/http")
local check = require("wrkr/check")
local json = require("wrkr/json")
local env = require("wrkr/env")

local base = env.BASE_URL
if base == nil then
  error("BASE_URL is required")
end
local echo_url = base .. "/echo"

local payload_tbl = {
  a = 1,
  b = 2,
  arr = { 1, 2, 3 },
  nested = { x = "y", z = true },
}

local expected_json = json.encode(payload_tbl)

local request_opts = {
  headers = {
    ["content-type"] = "application/json",
    ["accept"] = "application/json",
    ["x-test"] = "1",
  },
}

function Default()
  -- Encode once per-iteration to increase Lua-side CPU (json.encode uses serde_json).
  local body = json.encode(payload_tbl)

  local res = http.post(echo_url, body, request_opts)

  local decode_ok = false
  local decoded = nil
  if type(res.body) == "string" and res.body ~= "" then
    local ok, v = pcall(json.decode, res.body)
    decode_ok = ok
    if ok then
      decoded = v
    end
  end

  check(res, {
    ["status is 200"] = function(r)
      return r.status == 200
    end,
    ["echo body is expected"] = function(r)
      -- quick string equality check (server echoes exactly)
      return r.body == body
    end,
    ["encoded matches stable"] = function(_r)
      -- sanity: encoding is deterministic for this payload
      return body == expected_json
    end,
    ["response json parses"] = function(_r)
      return decode_ok
    end,
    ["decoded.arr[3] == 3"] = function(_r)
      return type(decoded) == "table" and type(decoded.arr) == "table" and decoded.arr[3] == 3
    end,
    ["decoded.nested.z == true"] = function(_r)
      return type(decoded) == "table" and type(decoded.nested) == "table" and decoded.nested.z == true
    end,
  })
end
