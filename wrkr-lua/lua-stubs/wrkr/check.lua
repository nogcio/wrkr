---@meta

---Run checks against any value.
---@generic T
---@param value T
---@param checks table<string, fun(value: T): boolean>
---@return boolean ok
return function(value, checks)
  return true
end
