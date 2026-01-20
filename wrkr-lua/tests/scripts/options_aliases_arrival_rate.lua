-- Regression test: options parsing should accept both camelCase and snake_case keys.

options = {
  scenarios = {
    camel = {
      executor = "ramping-arrival-rate",
      exec = "Default",

      startRate = 123,
      timeUnit = "250ms",
      preAllocatedVUs = 7,
      maxVUs = 99,

      stages = {
        { duration = "1s", target = 1000 },
        { duration = "2s", target = 10 },
      },
    },

    snake = {
      executor = "ramping-arrival-rate",
      exec = "Default",

      start_rate = 321,
      time_unit = "2s",
      pre_allocated_vus = 11,
      max_vus = 22,

      stages = {
        { duration = "3s", target = 33 },
      },
    },
  },
}
