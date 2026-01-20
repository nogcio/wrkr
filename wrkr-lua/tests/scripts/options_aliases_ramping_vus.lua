-- Regression test: options parsing should accept both camelCase and snake_case keys.

options = {
  scenarios = {
    camel = {
      executor = "ramping-vus",
      exec = "Default",

      startVUs = 3,
      stages = {
        { duration = "1s", target = 5 },
      },
    },

    snake = {
      executor = "ramping-vus",
      exec = "Default",

      start_vus = 4,
      stages = {
        { duration = "1s", target = 6 },
      },
    },
  },
}
