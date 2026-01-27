Options = {
  vus = 1,
  thresholds = {
    my_counter = "count == 0",
  },
}

local metrics = require("wrkr/metrics")

local c = metrics.Counter("my_counter")

function Default()
  c:add(1)
end
