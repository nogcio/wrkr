---@meta

local http = require("wrkr/http")
local grpc = require("wrkr/grpc")
local check = require("wrkr/check")
local env = require("wrkr/env")
local fs = require("wrkr/fs")
local debug = require("wrkr/debug")
local json = require("wrkr/json")
local uuid = require("wrkr/uuid")
local group = require("wrkr/group")
local metrics = require("wrkr/metrics")
local shared = require("wrkr/shared")
local vu = require("wrkr/vu")

---@class wrkr
---@field http wrkr.http
---@field grpc wrkr.grpc
---@field check fun(value: any, checks: table<string, fun(value: any): boolean>): boolean
---@field env wrkr.env
---@field fs wrkr.fs
---@field debug wrkr.debug
---@field json wrkr.json
---@field uuid wrkr.uuid
---@field group wrkr.group
---@field metrics wrkr.metrics
---@field shared wrkr.shared
---@field vu wrkr.vu
local M = {
  http = http,
  grpc = grpc,
  check = check,
  env = env,
  fs = fs,
  debug = debug,
  json = json,
  uuid = uuid,
  group = group,
  metrics = metrics,
  shared = shared,
  vu = vu,
}

return M
