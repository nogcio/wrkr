options = {
  vus = 1,
  iterations = 1,
}

function Setup()
  return nil
end

function Default()
  local shared = require("wrkr/shared")
  if shared.get("setup") ~= nil then
    error("expected nil setup data")
  end
end

function Teardown()
  local shared = require("wrkr/shared")
  if shared.get("setup") ~= nil then
    error("expected nil setup data in teardown")
  end
end
