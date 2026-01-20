options = {
  vus = 1,
  iterations = 1,
}

function Default(_data)
  local metrics = require("wrkr/metrics")
  metrics.Trend("")
end
