local env = require("wrkr/env")
local common = require("lib.wfb_common")

local M = {}

function M.require_env(key)
  local v = env[key]
  if v == nil or v == "" then
    error(key .. " is required")
  end
  return v
end

function M.base_url()
  return M.require_env("BASE_URL")
end

function M.duration(default)
  local d = env.WFB_DURATION
  if d == nil or d == "" then
    return default
  end
  return d
end

function M.max_vus(default)
  local v = tonumber(env.WFB_MAX_VUS or "")
  if v == nil or v < 1 then
    return default
  end
  return math.floor(v)
end

function M.parse_duration_seconds(d)
  if type(d) ~= "string" then
    return nil
  end

  local m = string.match(d, "^(%d+)%s*s$")
  if m ~= nil then
    return tonumber(m)
  end

  local n = tonumber(d)
  if n ~= nil then
    return n
  end

  return nil
end

function M.ramping_vus_options(vus, duration)
  local total = M.parse_duration_seconds(duration) or 10
  total = math.max(1, math.floor(total))

  -- Load profile: ramp up for 4/5 of the total duration, then hold
  -- the target VUs for the remaining 1/5.
  local up = math.max(1, math.floor(total * 0.8))
  local hold = total - up

  -- Ensure we have a non-zero hold phase when duration allows it.
  if total >= 2 and hold < 1 then
    hold = 1
    up = total - hold
  end

  local stages = {
    { duration = tostring(up) .. "s", target = vus },
  }
  if hold > 0 then
    table.insert(stages, { duration = tostring(hold) .. "s", target = vus })
  end

  return {
    scenarios = {
      main = {
        executor = "ramping-vus",
        startVUs = 0,
        stages = stages,
        exec = "Default",
      },
    },
  }
end

function M.to_num(v)
  return common.to_num(v)
end

function M.totals_match(actual_tbl, expected_tbl)
  return common.totals_match(actual_tbl, expected_tbl)
end

return M
