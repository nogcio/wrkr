Options = {
  vus = 1,
  iterations = 1,
}

function Default(_data)
  -- nothing
end

function HandleSummary(_summary)
  return {
    stdout = "hello\n",
    stderr = "oops\n",
    ["dir/out.txt"] = "ok\n",
  }
end
