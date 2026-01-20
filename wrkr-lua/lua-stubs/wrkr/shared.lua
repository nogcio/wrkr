---@meta

---@class wrkr.shared
local M = {}

---Get a value from the shared store.
---@param key string
---@return any|nil
function M.get(key)
	return nil
end

---Set a value in the shared store (JSON-encoded).
---@param key string
---@param value any
function M.set(key, value) end

---Delete a key (and any counter with the same name).
---@param key string
function M.delete(key) end

---Increment a shared counter and return its new value.
---@param key string
---@param delta? integer
---@return integer
function M.incr(key, delta)
	return 0
end

---Get a shared counter value (0 if missing).
---@param key string
---@return integer
function M.counter(key)
	return 0
end

---Wait until a key exists, then return its value.
---@async
---@param key string
---@return any
function M.wait(key)
	return nil
end

---Wait on a named barrier.
---@async
---@param name string
---@param parties integer
function M.barrier(name, parties)
	return nil
end

return M
