options = {
  scenarios = {
    main = {
      executor = "constant-vus",
      vus = 1,
      iterations = 1,
      tags = {
        env = "staging",
        build = 123,
        ok = true,
        group = "should_not_override_runtime_group",
      },
    },
  },
}

function Default()
  local env = require("wrkr/env")
  local http = require("wrkr/http")
  local group = require("wrkr/group")
  local check = require("wrkr/check")

  group.group("g_runtime", function()
    local res = http.get(env.BASE_URL .. "/plaintext", {
      tags = { route = "/plaintext" },
    })

    check(res, {
      ["http_ok"] = function(r) return r.status == 200 end,
    })
  end)
end
