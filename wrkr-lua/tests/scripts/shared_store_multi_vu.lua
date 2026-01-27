Options = { scenarios = { main = { vus = 2, iterations = 2, exec = 'Default' } } }

local shared = require("wrkr/shared")
local vu = require("wrkr/vu")

local function assert_eq(got, expected, msg)
  if got ~= expected then
    error((msg or "assert_eq failed") .. ": expected " .. tostring(expected) .. ", got " .. tostring(got))
  end
end

function Default()
  local id = vu.id()

  -- Synchronize startup so both VUs are active.
  shared.barrier("begin", 2)

  -- Counter across VUs (sync call).
  shared.incr("count", 1)

  -- Data written by VU1, read by both.
  if id == 1 then
    shared.set("arr", { 10, 20 })
    shared.set("obj", { a = true, b = "x", n = 1 })
    shared.set("data_ready", true)
  else
    local ok = shared.wait("data_ready")
    assert_eq(ok, true, "data_ready")
  end

  local arr = shared.get("arr")
  if type(arr) ~= "table" then
    error("shared.get(arr) expected table")
  end
  assert_eq(arr[1], 10, "arr[1]")
  assert_eq(arr[2], 20, "arr[2]")

  local obj = shared.get("obj")
  if type(obj) ~= "table" then
    error("shared.get(obj) expected table")
  end
  assert_eq(obj.a, true, "obj.a")
  assert_eq(obj.b, "x", "obj.b")
  assert_eq(obj.n, 1, "obj.n")

  -- Exercise wait on a key that is set AFTER the waiter signals readiness.
  if id == 2 then
    shared.set("waiter_ready", true)
    local v = shared.wait("ready")
    assert_eq(v, true, "ready")
  else
    local ok = shared.wait("waiter_ready")
    assert_eq(ok, true, "waiter_ready")
    shared.set("ready", true)
  end

  shared.barrier("after_wait", 2)

  -- Verify both increments are visible.
  local total = shared.counter("count")
  assert_eq(total, 2, "count")

  -- Ensure both VUs reached the end (regression guard for scheduling/iterations).
  shared.incr("done", 1)
  shared.barrier("done_barrier", 2)
  assert_eq(shared.counter("done"), 2, "done")
end
