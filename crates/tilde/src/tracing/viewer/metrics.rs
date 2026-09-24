//! Full-range aggregates. Payload searches use the observations API because the
//! Langfuse metrics API cannot execute its `matches` input/output filters.
use super::*;
use chrono::{DateTime, Datelike, Duration as TimeDelta, Months, NaiveDate, Utc};
use std::collections::BTreeSet;

type Totals = (pb::ObservationMetricBucket, f64, u64);

fn metric_time(value: &str) -> Result<DateTime<Utc>, ConnectError> {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc))
        .or_else(|_| {
            // Langfuse returns date-only keys for day/week/month buckets.
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        })
        .map_err(|_| ConnectError::unavailable("Invalid metrics timestamp"))
}
fn bucket_start(time: DateTime<Utc>, grain: &str) -> DateTime<Utc> {
    if grain == "month" {
        return time
            .date_naive()
            .with_day(1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
    }
    let (seconds, offset) = match grain {
        "minute" => (60, 0),
        "hour" => (3600, 0),
        "day" => (86400, 0),
        _ => (604800, 3 * 86400), // ISO weeks start on Monday.
    };
    DateTime::from_timestamp(
        (time.timestamp() + offset).div_euclid(seconds) * seconds - offset,
        0,
    )
    .unwrap()
}
fn bucket_end(time: DateTime<Utc>, grain: &str) -> DateTime<Utc> {
    match grain {
        "minute" => time + TimeDelta::minutes(1),
        "hour" => time + TimeDelta::hours(1),
        "day" => time + TimeDelta::days(1),
        "week" => time + TimeDelta::weeks(1),
        _ => time.checked_add_months(Months::new(1)).unwrap(),
    }
}
fn add(totals: &mut Totals, count: u64, errors: u64, latency_sum_ms: f64, latency_count: u64) {
    totals.0.observation_count += count;
    totals.0.error_count += errors;
    totals.1 += latency_sum_ms;
    totals.2 += latency_count;
}
impl Reader {
    pub async fn metrics(
        &self,
        agent: &str,
        filter: pb::ObservationFilter,
        refresh: bool,
    ) -> Result<pb::GetObservationMetricsResponse, ConnectError> {
        let agent = self.agent(agent).await?;
        let conditions = filter_conditions(&agent, &filter, None)?;
        // Reuse the table's normalized, validated range, including its 24h default.
        let from = DateTime::parse_from_rfc3339(conditions[1]["value"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let to = DateTime::parse_from_rfc3339(conditions[2]["value"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let length = to - from;
        let grain = if length <= TimeDelta::hours(2) {
            "minute"
        } else if length <= TimeDelta::days(3) {
            "hour"
        } else if length <= TimeDelta::days(90) {
            "day"
        } else if length <= TimeDelta::days(730) {
            "week"
        } else {
            "month"
        };
        let mut buckets = BTreeMap::<i64, Totals>::new();
        let mut start = bucket_start(from, grain);
        while start < to {
            if buckets.len() >= 200 {
                return Err(ConnectError::invalid_argument(
                    "Narrow the date range for the activity chart",
                ));
            }
            let end = bucket_end(start, grain);
            buckets.insert(
                start.timestamp(),
                (
                    pb::ObservationMetricBucket {
                        start_time: start.max(from).to_rfc3339(),
                        end_time: end.min(to).to_rfc3339(),
                        ..Default::default()
                    },
                    0.0,
                    0,
                ),
            );
            start = end;
        }
        if filter.input_search.is_empty() && filter.output_search.is_empty() {
            let filters: Vec<Value> = conditions
                .into_iter()
                .filter(|f| f["type"] != "datetime")
                .map(|mut f| {
                    if f["column"] == "model" {
                        f["column"] = json!("providedModelName");
                    }
                    f
                })
                .collect();
            let query = json!({
                "view":"observations", "dimensions":[{"field":"level"}],
                "metrics":[{"measure":"count","aggregation":"count"},{"measure":"latency","aggregation":"sum"},{"measure":"latency","aggregation":"count"}],
                "filters":filters, "timeDimension":{"granularity":grain},
                "fromTimestamp":from.to_rfc3339(), "toTimestamp":to.to_rfc3339(), "config":{"row_limit":1000}
            });
            let response = self
                .fetch(
                    "api/public/v2/metrics",
                    &[("query".into(), query.to_string())],
                    refresh,
                )
                .await?;
            let data = response["data"]
                .as_array()
                .ok_or_else(|| ConnectError::unavailable("Invalid metrics response"))?;
            if data.len() >= 1000 {
                return Err(ConnectError::resource_exhausted(
                    "Metrics response reached its limit; narrow the date range",
                ));
            }
            for row in data {
                let time = metric_time(&string(row, "time_dimension"))?;
                if let Some(bucket) = buckets.get_mut(&time.timestamp()) {
                    let count = number(row, "count_count").unwrap_or_default().max(0.0) as u64;
                    let samples = number(row, "count_latency").unwrap_or_default().max(0.0) as u64;
                    add(
                        bucket,
                        count,
                        if row["level"] == "ERROR" { count } else { 0 },
                        number(row, "sum_latency").unwrap_or_default(),
                        samples,
                    );
                }
            }
        } else {
            // Read only timing/scoping fields, not the payloads searched by Langfuse.
            // Deduplicate updated observations across all pages before aggregation.
            let mut rows = BTreeMap::<(String, String), Value>::new();
            let mut cursor = String::new();
            let mut cursors = BTreeSet::new();
            for page in 0..100 {
                let mut query = vec![
                    ("fields".into(), "core,basic,time,metadata,metrics".into()),
                    ("limit".into(), "1000".into()),
                    (
                        "filter".into(),
                        Value::Array(conditions.clone()).to_string(),
                    ),
                ];
                if !cursor.is_empty() {
                    query.push(("cursor".into(), cursor.clone()));
                }
                let response = self
                    .fetch("api/public/v2/observations", &query, refresh)
                    .await?;
                let data = response["data"]
                    .as_array()
                    .ok_or_else(|| ConnectError::unavailable("Invalid observations response"))?;
                for row in data {
                    if string(&row["metadata"], "tilde_agent_id") != agent {
                        continue;
                    }
                    let key = (string(row, "traceId"), string(row, "id"));
                    if rows
                        .get(&key)
                        .is_none_or(|old| string(row, "updatedAt") > string(old, "updatedAt"))
                    {
                        rows.insert(key, row.clone());
                    }
                }
                cursor = string(&response["meta"], "cursor");
                if cursor.is_empty() {
                    break;
                }
                if page == 99 || !cursors.insert(cursor.clone()) {
                    return Err(ConnectError::resource_exhausted(
                        "Content search is too large for the activity chart; narrow the date range",
                    ));
                }
            }
            for row in rows.values() {
                let Ok(time) = DateTime::parse_from_rfc3339(&string(row, "startTime")) else {
                    continue;
                };
                let time = time.with_timezone(&Utc);
                if time < from || time >= to {
                    continue;
                }
                if let Some(bucket) = buckets.get_mut(&bucket_start(time, grain).timestamp()) {
                    // Observations API reports seconds; Metrics API reports milliseconds.
                    let latency = number(row, "latency").filter(|v| *v >= 0.0);
                    add(
                        bucket,
                        1,
                        u64::from(row["level"] == "ERROR"),
                        latency.unwrap_or_default() * 1000.0,
                        u64::from(latency.is_some()),
                    );
                }
            }
        }
        Ok(pb::GetObservationMetricsResponse {
            buckets: buckets
                .into_values()
                .map(|(mut bucket, sum, count)| {
                    bucket.average_latency_seconds =
                        (count > 0).then(|| sum / count as f64 / 1000.0);
                    bucket
                })
                .collect(),
            ..Default::default()
        })
    }
}
