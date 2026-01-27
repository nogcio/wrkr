Options = {
  vus = 1,
  iterations = 1,
}

function Default(_data)
  -- nothing
end

function HandleSummary(_summary)
  return {
    ["../evil.txt"] = "nope\n",
  }
end
