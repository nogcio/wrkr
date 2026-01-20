---@meta

---@alias wrkr.MetricTags table<string, string|number|boolean>

---@class wrkr.TrendMetric
local TrendMetric = {}
---@param value number
---@param tags? wrkr.MetricTags
function TrendMetric:add(value, tags) end

---@class wrkr.CounterMetric
local CounterMetric = {}
---@param value number
---@param tags? wrkr.MetricTags
function CounterMetric:add(value, tags) end

---@class wrkr.GaugeMetric
local GaugeMetric = {}
---@param value number
---@param tags? wrkr.MetricTags
function GaugeMetric:add(value, tags) end

---@class wrkr.RateMetric
local RateMetric = {}
---@param value boolean
---@param tags? wrkr.MetricTags
function RateMetric:add(value, tags) end

---@class wrkr.metrics
local M = {}

---Create a Trend metric handle.
---@param name string
---@return wrkr.TrendMetric
function M.Trend(name) return TrendMetric end

---Create a Counter metric handle.
---@param name string
---@return wrkr.CounterMetric
function M.Counter(name) return CounterMetric end

---Create a Gauge metric handle.
---@param name string
---@return wrkr.GaugeMetric
function M.Gauge(name) return GaugeMetric end

---Create a Rate metric handle.
---@param name string
---@return wrkr.RateMetric
function M.Rate(name) return RateMetric end

return M
