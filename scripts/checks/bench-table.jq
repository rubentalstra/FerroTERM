# The README's speed and footprint table from one set of ferroterm-bench
# records (jq -n: every input file is one record, named by input_filename). Units fit the value:
# seconds, milliseconds, or microseconds; GB, MB, or KB.
def duration(ms):
  if ms == null then "n/a"
  elif ms >= 1000 then ((ms / 10 | round) / 100 | tostring) + " s"
  elif ms >= 1 then ((ms * 100 | round) / 100 | tostring) + " ms"
  else ((ms * 1000 | round) | tostring) + " µs"
  end;
def bytes(b):
  if b == null then "n/a"
  elif b >= 1000000000 then ((b / 10000000 | round) / 100 | tostring) + " GB"
  elif b >= 1000000 then ((b / 1000000) | round | tostring) + " MB"
  else ((b / 1000) | round | tostring) + " KB"
  end;
def op(r; name): (r.latency[] | select(.[0] == name) | .[1]) // null;
def cell(r; name): (op(r; name)) as $o | if $o == null then "n/a" else duration($o.p50_ms) end;
# A SNOMED CT version is its edition URI; the release date after "/version/" is
# what a reader wants in a column.
def release(v): if v == null then "n/a" elif (v | contains("/version/")) then (v | split("/version/")[1]) else v end;
def same(key): (map(key) | unique | length) == 1;

[inputs | . + {record: input_filename}] |
if (same(.machine) and same(.ferroterm_version) and same(.fhir)) | not then
  error("the records come from more than one machine, FerroTERM version, or FHIR version")
else
  (group_by(.system) | map(max_by(.taken_at)) | sort_by(.system)) as $rows |
  ($rows[0]) as $first |
  ([
    "| Code system | Release | Concepts | Build | Peak build memory | Index on disk | Resident | `$lookup` | `$validate-code` | `$subsumes` | `$expand` (small) | `$expand` (large) | Search | Snowstorm |",
    "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
  ]
  + ($rows | map(
      "| [" + .system + "](" + .record + ")"
      + " | " + release(.system_version)
      + " | " + ((.concepts // 0) | tostring)
      + " | " + (if .ingest == null then "n/a" else duration(.ingest.seconds * 1000) end)
      + " | " + (if .ingest == null then "n/a" else bytes(.ingest.peak_rss_bytes) end)
      + " | " + bytes(.artifact_bytes)
      + " | " + bytes(.rss_warm_bytes)
      + " | " + cell(.; "lookup")
      + " | " + cell(.; "validate-code")
      + " | " + cell(.; "subsumes")
      + " | " + cell(.; "expand-small")
      + " | " + cell(.; "expand-large")
      + " | " + cell(.; "search")
      + " | " + (.comparison // "not run")
      + " |"))
  + [
    "",
    "Warm p50 over " + ($first.latency[0][1].warm_requests | tostring) + " HTTP round trips on one machine" + (if $first.machine.container then ", inside a Docker container" else "" end) + " ("
      + $first.machine.cpu + ", " + bytes($first.machine.memory_bytes) + ", " + $first.machine.os + "/" + $first.machine.arch
      + "), FerroTERM " + $first.ferroterm_version + " serving FHIR " + ($first.fhir | ascii_upcase)
      + ", taken " + ($first.taken_at | .[0:10])
      + ". The records are under `bench/records/`; the [benchmarks page](https://ferroterm.eu/docs/evaluate/benchmarks.html) has the method, the cold and tail latencies, and how to reproduce a record."
  ]) | .[]
end
