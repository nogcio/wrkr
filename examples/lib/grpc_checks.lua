-- Checks helpers for gRPC examples.

local ex = require("lib.example")

local M = {}

local function to_num(v)
  if type(v) == "number" then
    return v
  end
  if type(v) == "string" then
    return tonumber(v) or 0
  end
  return 0
end

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

function M.grpc_ok()
  return function(v)
    local r = res_of(v)
    return type(r) == "table" and r.ok == true and r.status == 0
  end
end

function M.grpc_has_response_table()
  return function(v)
    local r = res_of(v)
    return type(r) == "table" and ex.table_is_table(r.response)
  end
end

function M.grpc_field_equals(field, expected_getter)
  return function(v)
    local r = res_of(v)
    if type(r) ~= "table" or not ex.table_is_table(r.response) then
      return false
    end
    return r.response[field] == expected_getter(v)
  end
end

function M.grpc_int_field_equals(field, expected_getter)
  return function(v)
    local r = res_of(v)
    if type(r) ~= "table" or not ex.table_is_table(r.response) then
      return false
    end
    return to_num(r.response[field]) == expected_getter(v)
  end
end

function M.grpc_totals_match(field, expected_getter)
  return function(v)
    local r = res_of(v)
    if type(r) ~= "table" or not ex.table_is_table(r.response) then
      return false
    end

    local actual = r.response[field]
    local expected = expected_getter(v)

    if type(actual) ~= "table" or type(expected) ~= "table" then
      return false
    end

    for k, expected_value in pairs(expected) do
      local actual_value = actual[k]
      if to_num(actual_value) ~= expected_value then
        return false
      end
    end

    return true
  end
end

return M
