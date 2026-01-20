options = { scenarios = { main = { vus = 1, iterations = 1, exec = 'Default' } } }

local shared = require("wrkr/shared")
local vu = require("wrkr/vu")

function Default()
  local id = vu.id()

  shared.set("arr", { 10, 20 })
  shared.set("obj", { a = true, b = "x", n = 1 })

  local arr = shared.get("arr")
  if type(arr) ~= "table" or arr[1] ~= 10 or arr[2] ~= 20 then
    error("shared.get(arr) mismatch")
  end

  local obj = shared.get("obj")
  if type(obj) ~= "table" or obj.a ~= true or obj.b ~= "x" or obj.n ~= 1 then
    error("shared.get(obj) mismatch")
  end

  local cur = shared.incr("count", 1)
  if type(cur) ~= "number" then
    error("shared.incr(count) did not return a number")
  end

  local total = shared.counter("count")
  if total ~= 1 then
    error("shared.counter(count) expected 1, got " .. tostring(total))
  end

  shared.set("ready", true)
  local ok = shared.wait("ready")
  if ok ~= true then
    error("shared.wait(ready) expected true, got " .. tostring(ok))
  end

  shared.barrier("noop", 1)

  shared.delete("arr")
  if shared.get("arr") ~= nil then
    error("shared.delete(arr) did not remove key")
  end
end
