---@meta

---@class wrkr.http
local M = {}

---Perform an HTTP GET.
---@param url string
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.get(url, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

---Perform an HTTP POST.
---@param url string
---@param body any
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.post(url, body, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

---Perform an HTTP PUT.
---@param url string
---@param body any
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.put(url, body, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

---Perform an HTTP PATCH.
---@param url string
---@param body any
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.patch(url, body, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

---Perform an HTTP DELETE.
---@param url string
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.delete(url, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

---Perform an HTTP HEAD.
---@param url string
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.head(url, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

---Perform an HTTP OPTIONS.
---@param url string
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.options(url, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

---Perform an HTTP request with a custom method.
---
---If `body` is a string, it is sent as-is.
---Otherwise, it is JSON-encoded.
---@param method string e.g. "GET", "POST"
---@param url string
---@param body? any
---@param opts? wrkr.HttpRequestOptions
---@async
---@return wrkr.HttpResponse
function M.request(method, url, body, opts)
  return { status = 200, body = "", headers = {}, error = nil }
end

return M
