Options = {
  vus = 1,
  iterations = 1,
  thresholds = {
    ["my_counter{ group=login, method = GET }"] = "count>0",
  },
}

function Default(_data)
  -- No-op; this script only validates options parsing.
end
