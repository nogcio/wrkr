---@meta

---@class wrkr.debug
local M = {}

---Attempt to start a VS Code Lua debugger (e.g. local-lua-debugger).
---Returns true if it looks like the debugger started.
---@return boolean started
function M.start()
  return false
end

---Start the debugger if the process environment indicates VS Code debugging.
function M.maybe_start()
end

return M
