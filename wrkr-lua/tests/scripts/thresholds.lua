options = {
  vus = 1,
  iterations = 1,
  thresholds = {
    http_req_duration = { "avg<0" },
  },
}

function Default(_data)
  local env = require("wrkr/env")
  local http = require("wrkr/http")

  local res = http.get(env.BASE_URL .. "/plaintext", { name = "GET /plaintext" })
  if res.status ~= 200 then
    error("unexpected status: " .. tostring(res.status))
  end
end
