local M = {}

function M.to_num(v)
  if type(v) == "number" then
    return v
  end
  if type(v) == "string" then
    return tonumber(v) or 0
  end
  return 0
end

function M.totals_match(actual_tbl, expected_tbl)
  if type(actual_tbl) ~= "table" or type(expected_tbl) ~= "table" then
    return false
  end

  for k, expected_value in pairs(expected_tbl) do
    local actual_value = actual_tbl[k]
    if M.to_num(actual_value) ~= expected_value then
      return false
    end
  end

  return true
end

return M
