local M = {}

local Pool = {}
Pool.__index = Pool

function M.new(opts)
  if type(opts) ~= "table" then
    error("pool.new(opts): opts must be a table")
  end
  if type(opts.size) ~= "number" or opts.size < 1 then
    error("pool.new(opts): opts.size must be a positive number")
  end
  if type(opts.generate) ~= "function" then
    error("pool.new(opts): opts.generate must be a function")
  end

  local seed = opts.seed
  if seed ~= nil and type(seed) ~= "function" then
    error("pool.new(opts): opts.seed must be a function")
  end

  return setmetatable({
    size = opts.size,
    generate = opts.generate,
    seed = seed,
    items = {},
    index = 1,
    initialized = false,
  }, Pool)
end

function Pool:ensure_initialized(vu_id)
  if self.initialized then
    return
  end

  if self.seed ~= nil then
    self.seed(vu_id or 0)
  else
    math.randomseed(os.time() + (vu_id or 0))
  end

  for i = 1, self.size do
    self.items[i] = self.generate()
  end

  self.initialized = true
end

function Pool:next()
  local item = self.items[self.index]
  self.index = (self.index % self.size) + 1
  return item
end

return M
