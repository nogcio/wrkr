-- wrk script: POST /echo with JSON body + response validation.
-- URL passed to wrk should be the BASE_URL (e.g. http://127.0.0.1:1234)

wrk.method = "POST"
wrk.path = "/echo"
wrk.headers["Content-Type"] = "application/json"
wrk.headers["Accept"] = "application/json"
wrk.headers["x-test"] = "1"

-- Deterministic JSON payload.
local payload = "{\"a\":1,\"b\":2,\"arr\":[1,2,3],\"nested\":{\"x\":\"y\",\"z\":true}}"

-- Tiny Lua CPU work: checksum.
local function checksum(s)
  local c = 0
  for i = 1, #s do
    c = (c + string.byte(s, i)) % 2147483647
  end
  return c
end

local expected_sum = checksum(payload)

local errors = 0

request = function()
  return wrk.format(nil, nil, nil, payload)
end

response = function(status, _headers, body)
  if status ~= 200 then
    errors = errors + 1
    return
  end

  if body ~= payload then
    errors = errors + 1
    return
  end

  if checksum(body) ~= expected_sum then
    errors = errors + 1
    return
  end
end

done = function(summary, _latency, _requests)
  io.write(string.format("lua_errors: %d\n", errors))
end
