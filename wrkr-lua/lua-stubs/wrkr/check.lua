---@meta

---Run checks against a response.
---@param res wrkr.HttpResponse
---@param checks table<string, fun(res: wrkr.HttpResponse): boolean>
---@return boolean ok
return function(res, checks)
  return true
end
