---@meta

---@class wrkr.HttpResponse
---@field status integer HTTP status code, or 0 on transport error
---@field body string Response body decoded as UTF-8 (may be empty)
---@field headers table<string, string> Response headers (lowercased header names)
---@field error? string Error message (present when status==0)

---@class wrkr.HttpRequestOptions
---@field headers? table<string, string|number>
---@field params? table<string, string|number>
---@field timeout? number|string Timeout in seconds or duration string (e.g. "250ms", "10s")
---@field tags? table<string, string|number|boolean>
---@field name? string

---@class wrkr.CheckSummary
---@field name string
---@field total integer
---@field failed integer
---@field succeeded integer

---@class wrkr.LatencyDistributionPoint
---@field p integer
---@field value integer

---@alias wrkr.MetricKind "trend"|"counter"|"gauge"|"rate"

---@class wrkr.MetricValuesTrend
---@field count integer
---@field min number
---@field max number
---@field avg number
---@field p50 number
---@field p90 number
---@field p95 number
---@field p99 number

---@class wrkr.MetricValuesCounter
---@field value number

---@class wrkr.MetricValuesGauge
---@field value number

---@class wrkr.MetricValuesRate
---@field total integer
---@field trues integer
---@field rate number

---@alias wrkr.MetricValues wrkr.MetricValuesTrend|wrkr.MetricValuesCounter|wrkr.MetricValuesGauge|wrkr.MetricValuesRate

---@class wrkr.MetricSeriesSummary
---@field name string
---@field type wrkr.MetricKind
---@field tags table<string, string>
---@field values wrkr.MetricValues

---@class wrkr.RunSummary
---@field requests_total integer
---@field checks_total integer
---@field checks_failed integer
---@field checks_succeeded integer
---@field checks_by_name wrkr.CheckSummary[]
---@field dropped_iterations_total integer
---@field bytes_received_total integer
---@field bytes_sent_total integer
---@field run_duration integer
---@field rps number
---@field req_per_sec_avg number
---@field req_per_sec_stdev number
---@field req_per_sec_max number
---@field req_per_sec_stdev_pct number
---@field latency_p50 number|nil
---@field latency_p75 number|nil
---@field latency_p90 number|nil
---@field latency_p95 number|nil
---@field latency_p99 number|nil
---@field latency_mean number|nil
---@field latency_stdev number|nil
---@field latency_max integer|nil
---@field latency_distribution wrkr.LatencyDistributionPoint[]
---@field metrics wrkr.MetricSeriesSummary[]
