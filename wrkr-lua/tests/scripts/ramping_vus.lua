Options = {
  scenarios = {
    main = {
      executor = 'ramping-vus',
      startVUs = 0,
      stages = {
        { duration = '50ms', target = 2 },
        { duration = '50ms', target = 0 },
      },
      exec = 'Default',
    },
  },
}

local http = require('wrkr/http')
local check = require('wrkr/check')
local env = require('wrkr/env')

function Default()
  local base = env.BASE_URL
  local res = http.get(base .. '/plaintext')
  check(res, {
    ['status is 200'] = function(r) return r.status == 200 end,
  })
end
