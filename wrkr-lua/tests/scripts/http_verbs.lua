Options = {
  scenarios = {
    main = {
      executor = "constant-vus",
      vus = 1,
      iterations = 1,
      exec = "Default",
    },
  },
}

function Default()
  local env = require("wrkr/env")
  local http = require("wrkr/http")
  local group = require("wrkr/group")

  group.group("g_http_verbs", function()
    local base = env.BASE_URL

    local opts = { tags = { route = "/echo" } }

    local res_put = http.put(base .. "/echo", "ping", {
      name = "PUT /echo",
      tags = opts.tags,
    })
    if res_put.status ~= 200 then
      error("unexpected PUT status: " .. tostring(res_put.status))
    end

    local res_patch = http.patch(base .. "/echo", "ping", {
      name = "PATCH /echo",
      tags = opts.tags,
    })
    if res_patch.status ~= 200 then
      error("unexpected PATCH status: " .. tostring(res_patch.status))
    end

    local res_delete = http.delete(base .. "/echo", {
      name = "DELETE /echo",
      tags = opts.tags,
    })
    if res_delete.status ~= 200 then
      error("unexpected DELETE status: " .. tostring(res_delete.status))
    end

    local res_head = http.head(base .. "/echo", {
      name = "HEAD /echo",
      tags = opts.tags,
    })
    if res_head.status ~= 200 then
      error("unexpected HEAD status: " .. tostring(res_head.status))
    end
  end)
end
