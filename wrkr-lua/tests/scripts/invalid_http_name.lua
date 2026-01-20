options = {
  vus = 1,
  iterations = 1,
}

function Default(_data)
  local env = require("wrkr/env")
  local http = require("wrkr/http")

  local _ = http.get(env.BASE_URL .. "/plaintext", { name = 123 })
end
