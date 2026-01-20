---@meta

---@class wrkr.http
local M = {}

---Perform an HTTP GET.
---@param url string
---@param opts? wrkr.HttpRequestOptions
---@return wrkr.HttpResponse
function M.get(url, opts)
  return { status = 200, body = "", error = nil }
end

---Perform an HTTP POST.
---@param url string
---@param body any
---@param opts? wrkr.HttpRequestOptions
---@return wrkr.HttpResponse
function M.post(url, body, opts)
  return { status = 200, body = "", error = nil }
end

return M
