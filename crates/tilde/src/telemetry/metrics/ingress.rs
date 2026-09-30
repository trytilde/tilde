//! Validate OTLP metrics before batching and replace ownership using authenticated context.
//! Ownership is stamped on every data point, which is what the metric tables index.
use opentelemetry_proto::tonic::{
    common::v1::{AnyValue, KeyValue, any_value::Value},
    metrics::v1::{ResourceMetrics, metric::Data},
};
use prost::Message as _;
use rotel::topology::payload::{Message, RequestContext};
use uuid::Uuid;

pub fn validate(message: &Message<ResourceMetrics>) -> Result<(), u16> {
    let Some(RequestContext::Http(headers)) = &message.request_context else {
        return Err(400);
    };
    if !matches!(
        headers.get("x-tilde-log-scope").map(String::as_str),
        Some("deployment") | Some("invocation") | None
    ) {
        return Err(400);
    }
    let mut points = 0;
    for resource in &message.payload {
        for scope in &resource.scope_metrics {
            for metric in &scope.metrics {
                if metric.encoded_len() > 1024 * 1024 || metric.name.is_empty() {
                    return Err(400);
                }
                points += match &metric.data {
                    Some(Data::Gauge(g)) => g.data_points.len(),
                    Some(Data::Sum(s)) => s.data_points.len(),
                    Some(Data::Histogram(h)) => h.data_points.len(),
                    Some(Data::ExponentialHistogram(h)) => h.data_points.len(),
                    Some(Data::Summary(s)) => s.data_points.len(),
                    None => return Err(400),
                };
                if points > 16384 {
                    return Err(400);
                }
            }
        }
    }
    Ok(())
}
/// Every attribute list a data point carries, whatever its kind.
fn attribute_lists(resources: &mut [ResourceMetrics]) -> Vec<&mut Vec<KeyValue>> {
    let mut lists = Vec::new();
    for resource in resources {
        if let Some(r) = &mut resource.resource {
            lists.push(&mut r.attributes);
        }
        for scope in &mut resource.scope_metrics {
            if let Some(s) = &mut scope.scope {
                lists.push(&mut s.attributes);
            }
            for metric in &mut scope.metrics {
                match &mut metric.data {
                    Some(Data::Gauge(g)) => {
                        lists.extend(g.data_points.iter_mut().map(|p| &mut p.attributes))
                    }
                    Some(Data::Sum(s)) => {
                        lists.extend(s.data_points.iter_mut().map(|p| &mut p.attributes))
                    }
                    Some(Data::Histogram(h)) => {
                        lists.extend(h.data_points.iter_mut().map(|p| &mut p.attributes))
                    }
                    Some(Data::ExponentialHistogram(h)) => {
                        lists.extend(h.data_points.iter_mut().map(|p| &mut p.attributes))
                    }
                    Some(Data::Summary(s)) => {
                        lists.extend(s.data_points.iter_mut().map(|p| &mut p.attributes))
                    }
                    None => {}
                }
            }
        }
    }
    lists
}
pub fn prepare(message: &mut Message<ResourceMetrics>) -> Result<String, u16> {
    let Some(RequestContext::Http(headers)) = &message.request_context else {
        return Err(400);
    };
    let scope = headers
        .get("x-tilde-log-scope")
        .map(String::as_str)
        .unwrap_or("invocation");
    let mut ownership = vec![("x-tilde-trace-agent", "tilde.agent.id")];
    match scope {
        "deployment" => ownership.push(("x-tilde-deployment", "tilde.deployment.id")),
        "invocation" => ownership.extend([
            ("x-tilde-trace-invocation", "tilde.invocation.id"),
            ("x-tilde-trace-run", "tilde.run.id"),
            ("x-tilde-trace-thread", "tilde.thread.id"),
        ]),
        _ => return Err(400),
    }
    let mut attributes = Vec::new();
    for (header, key) in ownership {
        let value = headers.get(header).ok_or(400u16)?;
        Uuid::parse_str(value).map_err(|_| 400u16)?;
        attributes.push(KeyValue {
            key: key.into(),
            value: Some(AnyValue {
                value: Some(Value::StringValue(value.clone())),
            }),
            ..Default::default()
        });
    }
    let agent = headers["x-tilde-trace-agent"].clone();
    for list in attribute_lists(&mut message.payload) {
        list.retain(|a| !a.key.starts_with("tilde."));
    }
    // Resource and scope lists were cleared above; only data points receive ownership.
    stamp(&mut message.payload, &attributes);
    Ok(agent)
}
/// Ownership on every data point, as the gateway re-stamps relayed batches too.
pub fn stamp(resources: &mut [ResourceMetrics], attributes: &[KeyValue]) {
    for resource in resources {
        for scope in &mut resource.scope_metrics {
            for metric in &mut scope.metrics {
                let points: Vec<&mut Vec<KeyValue>> = match &mut metric.data {
                    Some(Data::Gauge(g)) => g
                        .data_points
                        .iter_mut()
                        .map(|p| &mut p.attributes)
                        .collect(),
                    Some(Data::Sum(s)) => s
                        .data_points
                        .iter_mut()
                        .map(|p| &mut p.attributes)
                        .collect(),
                    Some(Data::Histogram(h)) => h
                        .data_points
                        .iter_mut()
                        .map(|p| &mut p.attributes)
                        .collect(),
                    Some(Data::ExponentialHistogram(h)) => h
                        .data_points
                        .iter_mut()
                        .map(|p| &mut p.attributes)
                        .collect(),
                    Some(Data::Summary(s)) => s
                        .data_points
                        .iter_mut()
                        .map(|p| &mut p.attributes)
                        .collect(),
                    None => Vec::new(),
                };
                for list in points {
                    list.retain(|a| !attributes.iter().any(|stamped| stamped.key == a.key));
                    list.extend(attributes.iter().cloned());
                }
            }
        }
    }
}
