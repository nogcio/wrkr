---@meta

---@class wrkr.group
local M = {}

---Run a function within a named group (used for tagging metrics like HTTP and custom metrics).
---@async
---@param name string
---@param f fun(): any
---@return any
function M.group(name, f)
  return nil
end

return M
