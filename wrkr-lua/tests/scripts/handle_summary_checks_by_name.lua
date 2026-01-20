options = {
  vus = 1,
  iterations = 1,
}

local check = require("wrkr/check")
local json = require("wrkr/json")

function Default()
  local res = { status = 200, body = "" }

  check(res, {
    ["check ok"] = function(_r)
      return true
    end,
    ["check fail"] = function(_r)
      return false
    end,
  })
end

function HandleSummary(summary)
  return {
    ["summary.json"] = json.encode(summary) .. "\n",
  }
end
