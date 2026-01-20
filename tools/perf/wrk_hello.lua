-- wrk script: GET /hello with minimal overhead.
-- URL passed to wrk should be the BASE_URL (e.g. http://127.0.0.1:1234)

wrk.method = "GET"
wrk.path = "/hello"
wrk.headers["Accept"] = "*/*"

request = function()
  return wrk.format(nil, nil, nil, nil)
end
