//! Full-range aggregates for the activity chart. One query covers every filter, including
//! input and output search, because the span store evaluates them all.
use super::*;
use chrono::{DateTime, Datelike, Duration as TimeDelta, Months, Utc};

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
impl Reader {
    pub async fn metrics(
        &self,
        agent: &str,
        filter: pb::ObservationFilter,
    ) -> Result<pb::GetObservationMetricsResponse, ConnectError> {
        let agent = self.agent(agent).await?;
        let mut query = Self::query(&agent, &filter, None)?;
        let from = DateTime::from_timestamp_nanos(query.from);
        let to = DateTime::from_timestamp_nanos(query.to);
        let range = to - from;
        let grain = if range <= TimeDelta::hours(2) {
            "minute"
        } else if range <= TimeDelta::days(3) {
            "hour"
        } else if range <= TimeDelta::days(90) {
            "day"
        } else if range <= TimeDelta::days(730) {
            "week"
        } else {
            "month"
        };
        // Every interval is emitted, so quiet periods show as gaps rather than disappearing.
        let mut buckets = BTreeMap::new();
        let mut start = bucket_start(from, grain);
        while start < to {
            let end = bucket_end(start, grain);
            buckets.insert(
                start.timestamp(),
                pb::ObservationMetricBucket {
                    start_time: start.max(from).to_rfc3339(),
                    end_time: end.min(to).to_rfc3339(),
                    ..Default::default()
                },
            );
            if buckets.len() > 200 {
                return Err(ConnectError::invalid_argument(
                    "Narrow the date range for the activity chart",
                ));
            }
            start = end;
        }
        query.parameters.push(parameter("grain", grain));
        let sql = include_str!("../../../../../../queries/_traces/metrics.sql")
            .replace("%%CONDITIONS%%", &query.clauses);
        for row in self.run(&sql, &query.parameters).await? {
            let Some(bucket) = text(&row, "bucket")
                .parse::<i64>()
                .ok()
                .and_then(|at| buckets.get_mut(&at))
            else {
                continue;
            };
            let observations = count(&row["observations"]).unwrap_or_default();
            bucket.observation_count = observations;
            bucket.error_count = count(&row["errors"]).unwrap_or_default();
            bucket.average_latency_seconds = (observations > 0).then(|| {
                text(&row, "duration_ns").parse::<f64>().unwrap_or_default()
                    / observations as f64
                    / 1e9
            });
        }
        Ok(pb::GetObservationMetricsResponse {
            buckets: buckets.into_values().collect(),
            ..Default::default()
        })
    }
}
