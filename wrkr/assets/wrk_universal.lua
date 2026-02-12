local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")
local json = require("wrkr/json")

local function getenv(key)
  local v = env[key]
  if v == nil then
    return nil
  end
  v = tostring(v)
  if v == "" then
    return nil
  end
  return v
end

local function headers_from_env()
  local raw = getenv("WRKR_REQUEST_HEADERS_JSON")
  if raw == nil then
    return {}
  end

  local decoded = json.decode(raw)
  if decoded == nil then
    return {}
  end

  local headers = {}

  -- Expect an array of {k=..., v=...} objects.
  for _, kv in ipairs(decoded) do
    if type(kv) == "table" then
      local k = kv.k
      local v = kv.v
      if k ~= nil and v ~= nil then
        headers[tostring(k)] = tostring(v)
      end
    end
  end

  return headers
end

local METHOD = getenv("WRKR_REQUEST_METHOD") or "GET"
local URL = assert(getenv("WRKR_REQUEST_URL"), "WRKR_REQUEST_URL is required")

local OPTS = {
  headers = headers_from_env(),
}

local timeout = getenv("WRKR_REQUEST_TIMEOUT")
if timeout ~= nil then
  OPTS.timeout = timeout
end

local name = getenv("WRKR_REQUEST_NAME")
if name ~= nil then
  OPTS.name = name
end

local body_present = getenv("WRKR_REQUEST_BODY_PRESENT")
local BODY = nil
if body_present == "1" then
  BODY = getenv("WRKR_REQUEST_BODY") or ""
end

function Default()
  local res = http.request(METHOD, URL, BODY, OPTS)
  check(res, {
    ["no 3xx"] = function(r)
      local s = type(r) == "table" and r.status or nil
      return type(s) == "number" and not (s >= 300 and s < 400)
    end,
    ["no 4xx"] = function(r)
      local s = type(r) == "table" and r.status or nil
      return type(s) == "number" and not (s >= 400 and s < 500)
    end,
    ["no 5xx"] = function(r)
      local s = type(r) == "table" and r.status or nil
      return type(s) == "number" and not (s >= 500 and s < 600)
    end,
  })
end
