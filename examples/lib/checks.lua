-- Designed to work with either:
--  - a raw HTTP response table { status, body, ... }
--  - a context table { res = <response>, ... }

local ex = require("lib.example")

local M = {}

local function res_of(v)
  if type(v) == "table" and v.res ~= nil then
    return v.res
  end
  return v
end

function M.with_res(ctx, res)
  ctx.res = res
  return ctx
end

function M.json_ctx(res, json)
  local decode_ok, decoded = pcall(json.decode, res.body)
  return {
    res = res,
    decode_ok = decode_ok,
    body = decoded,
  }
end

function M.status_is(code)
  return function(v)
    local r = res_of(v)
    return type(r) == "table" and r.status == code
  end
end

function M.body_equals(expected)
  return function(v)
    local r = res_of(v)
    return type(r) == "table" and r.body == expected
  end
end

function M.json_decode_ok()
  return function(v)
    return type(v) == "table" and v.decode_ok == true
  end
end

function M.json_body_is_object()
  return function(v)
    return type(v) == "table" and v.decode_ok == true and ex.table_is_table(v.body)
  end
end

function M.json_field_equals(field, expected_getter)
  return function(v)
    if type(v) ~= "table" or v.decode_ok ~= true or not ex.table_is_table(v.body) then
      return false
    end
    local expected = expected_getter(v)
    return v.body[field] == expected
  end
end

function M.json_totals_match(field, expected_getter)
  return function(v)
    if type(v) ~= "table" or v.decode_ok ~= true or not ex.table_is_table(v.body) then
      return false
    end
    local expected = expected_getter(v)
    return ex.totals_match(v.body[field], expected)
  end
end

return M
