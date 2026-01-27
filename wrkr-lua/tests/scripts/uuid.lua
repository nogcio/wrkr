Options = { iterations = 1 }

local uuid = require("wrkr/uuid")
local check = require("wrkr/check")

function Default()
  local a = uuid.v4()
  local b = uuid.v4()

  check({ a = a, b = b }, {
    ["v4 returns string"] = function(x)
      return type(x.a) == "string" and #x.a == 36
    end,
    ["v4 is random"] = function(x)
      return x.a ~= x.b
    end,
  })
end
