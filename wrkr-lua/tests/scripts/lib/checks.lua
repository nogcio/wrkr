local M = {}

function M.status_is(res, expected)
  return res.status == expected
end

function M.body_is(res, expected)
  return res.body == expected
end

return M
