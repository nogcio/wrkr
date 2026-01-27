Options = {
  scenarios = {
    main = {
      executor = "constant-vus",
      vus = 1,
      duration = "50ms",
      tags = {
        env = "staging",
        build = 123,
        ok = true,
        group = "should_not_override_runtime_group",
      },
    },
  },
}

function Default()
end
