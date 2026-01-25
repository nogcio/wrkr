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
  local group = require("wrkr/group")
  local check = require("wrkr/check")
  local metrics = require("wrkr/metrics")

  local counter = metrics.Counter("custom_counter_scenario_tags")

  group.group("g_runtime", function()
    check({}, {
      ["ok"] = function(_) return true end,
    })

    counter:add(1, { k = "v" })
  end)
end
