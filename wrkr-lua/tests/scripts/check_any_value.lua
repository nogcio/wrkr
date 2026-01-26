local check = require("wrkr/check")

options = { vus = 1, iterations = 1 }

function Default()
  check("hello", {
    ["is hello"] = function(v)
      return v == "nope"
    end,
  })
end
