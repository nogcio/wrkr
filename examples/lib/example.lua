-- Shared helpers for examples/* scripts.

local env = require("wrkr/env")

local M = {}

function M.base_url()
  local base = env.BASE_URL
  if base == nil then
    error("BASE_URL is required")
  end
  return base
end

function M.table_is_table(v)
  return type(v) == "table"
end

-- Compare expected numeric totals with actual totals.
-- Missing keys in actual are treated as 0.
function M.totals_match(actual_tbl, expected_tbl)
  if type(actual_tbl) ~= "table" then
    return false
  end

  for k, expected_value in pairs(expected_tbl) do
    local actual_value = actual_tbl[k] or 0
    if actual_value ~= expected_value then
      return false
    end
  end

  return true
end

return M
