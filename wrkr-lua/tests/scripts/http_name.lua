options = {
  scenarios = {
    main = {
      executor = "constant-vus",
      vus = 1,
      iterations = 1,
      tags = {
        env = "staging",
        build = 123,
      },
    },
  },
}

function Default()
  local env = require("wrkr/env")
  local http = require("wrkr/http")
  local group = require("wrkr/group")

  group.group("g_http", function()
    local res = http.get(env.BASE_URL .. "/plaintext", {
      name = "GET /plaintext",
      tags = { route = "/plaintext" },
    })

    if res.status ~= 200 then
      error("unexpected status: " .. tostring(res.status))
    end
  end)
end
