-- The whole filtered range, independent of list pages, with the same predicates as list.sql.
-- Calendar buckets in UTC; weeks start on Monday.
SELECT toString(toUnixTimestamp(multiIf(
  {grain:String} = 'minute', toStartOfMinute(Timestamp, 'UTC'),
  {grain:String} = 'hour', toStartOfHour(Timestamp, 'UTC'),
  {grain:String} = 'day', toDateTime(toStartOfDay(Timestamp, 'UTC'), 'UTC'),
  {grain:String} = 'week', toDateTime(toStartOfWeek(Timestamp, 1, 'UTC'), 'UTC'),
  toDateTime(toStartOfMonth(Timestamp, 'UTC'), 'UTC')))) AS bucket,
 count() AS observations, countIf(Level = 'ERROR') AS errors, toString(sum(Duration)) AS duration_ns
FROM otel_traces FINAL
WHERE AgentId = {agent:String}
 AND Timestamp >= fromUnixTimestamp64Nano({from:Int64})
 AND Timestamp < fromUnixTimestamp64Nano({to:Int64})
%%CONDITIONS%%GROUP BY bucket ORDER BY bucket
SETTINGS max_execution_time = 10, max_result_bytes = 33554432, result_overflow_mode = 'throw', max_memory_usage = 536870912
FORMAT JSON
