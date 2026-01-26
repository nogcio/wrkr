---@meta

local M = {}

M.check = require("wrkr/check")
M.env = require("wrkr/env")
M.fs = require("wrkr/fs")
M.debug = require("wrkr/debug")
M.json = require("wrkr/json")
M.uuid = require("wrkr/uuid")
M.group = require("wrkr/group")
M.metrics = require("wrkr/metrics")
M.shared = require("wrkr/shared")
M.vu = require("wrkr/vu")

local ok_http, http = pcall(require, "wrkr/http")
if ok_http then
  M.http = http
end

local ok_grpc, grpc = pcall(require, "wrkr/grpc")
if ok_grpc then
  M.grpc = grpc
end

return M
