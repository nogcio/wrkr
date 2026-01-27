Options = {
  vus = 1,
  iterations = 1,
  thresholds = {
    -- Tag-scoped threshold: only counts samples recorded under group=login.
    ["my_counter{group=login}"] = "count==1",
  },
}

local group = require("wrkr/group")
local metrics = require("wrkr/metrics")

local c = metrics.Counter("my_counter")

function Default()
  group.group("login", function()
    c:add(1)
  end)
end
