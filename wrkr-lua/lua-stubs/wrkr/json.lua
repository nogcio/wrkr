---@meta

---@class wrkr.json
local M = {}

---Encode a Lua value as JSON.
---@param value any
---@return string
function M.encode(value)
  return "{}"
end

---Decode a JSON string into Lua values (tables/numbers/strings/bools/nil).
---@param json string
---@return any
function M.decode(json)
  return {}
end

return M
