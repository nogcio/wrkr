Options = {
  vus = 1,
  iterations = 1,
}

function Default()
  local env = require("wrkr/env")
  local http = require("wrkr/http")

  local res = http.get(env.BASE_URL .. "/plaintext")
  if res.status ~= 200 then
    error("unexpected status: " .. tostring(res.status))
  end

  if res.headers == nil then
    error("expected res.headers")
  end

  local ct = res.headers["content-type"]
  if ct == nil then
    error("expected content-type header")
  end
end
