//! The observation filter model rendered as ClickHouse predicates. Columns and
//! operators are matched against fixed vocabularies and every value is a query parameter, so
//! the SQL text a request can influence is limited to which whitelisted clauses appear.
use crate::proto::tilde::management::v1 as pb;
use connectrpc::ConnectError;
use sha2::{Digest, Sha256};

pub const MAX_CONDITIONS: usize = 32;
const MAX_VALUE: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Number,
    Attribute,
}
fn column(name: &str) -> Option<(&'static str, Kind)> {
    Some(match name {
        "name" => ("SpanName", Kind::Text),
        "type" => ("ObservationType", Kind::Text),
        "level" => ("Level", Kind::Text),
        "model" => ("Model", Kind::Text),
        "session_id" | "sessionId" => ("ThreadId", Kind::Text),
        "invocation_id" | "invocationId" => ("InvocationId", Kind::Text),
        "status_message" | "statusMessage" => ("StatusMessage", Kind::Text),
        "input" => ("SpanAttributes['tilde.observation.input']", Kind::Text),
        "output" => ("SpanAttributes['tilde.observation.output']", Kind::Text),
        "latency" => ("(Duration / 1e9)", Kind::Number),
        "total_tokens" | "totalTokens" => (
            "(toFloat64OrZero(SpanAttributes['gen_ai.usage.input_tokens']) + toFloat64OrZero(SpanAttributes['gen_ai.usage.output_tokens']))",
            Kind::Number,
        ),
        "input_tokens" | "inputTokens" => (
            "toFloat64OrNull(SpanAttributes['gen_ai.usage.input_tokens'])",
            Kind::Number,
        ),
        "output_tokens" | "outputTokens" => (
            "toFloat64OrNull(SpanAttributes['gen_ai.usage.output_tokens'])",
            Kind::Number,
        ),
        "total_cost" | "totalCost" => (
            "toFloat64OrNull(SpanAttributes['tilde.observation.cost_usd'])",
            Kind::Number,
        ),
        "time_to_first_token" | "timeToFirstToken" => (
            "(toUnixTimestamp64Nano(parseDateTime64BestEffortOrNull(SpanAttributes['tilde.observation.completion_start_time'], 9)) - toUnixTimestamp64Nano(Timestamp)) / 1e9",
            Kind::Number,
        ),
        "metadata" => ("SpanAttributes[{key}]", Kind::Attribute),
        _ => return None,
    })
}
/// A literal substring for ILIKE.
fn pattern(value: &str, before: &str, after: &str) -> String {
    format!(
        "{before}{}{after}",
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}
/// ClickHouse Array(String) literal for an HTTP parameter.
fn array(values: &[String]) -> String {
    let quoted: Vec<_> = values
        .iter()
        .map(|v| format!("'{}'", v.replace('\\', "\\\\").replace('\'', "\\'")))
        .collect();
    format!("[{}]", quoted.join(","))
}
pub struct Rendered {
    /// `AND (...)` clauses to splice after the fixed agent and time predicates.
    pub clauses: String,
    pub parameters: Vec<(String, String)>,
    /// Identifies the conditions for cursors.
    pub identity: String,
}
pub fn render(conditions: &[pb::FilterCondition]) -> Result<Rendered, ConnectError> {
    if conditions.len() > MAX_CONDITIONS {
        return Err(ConnectError::invalid_argument("Too many filter conditions"));
    }
    let invalid = |message: &'static str| ConnectError::invalid_argument(message);
    let mut clauses = String::new();
    let mut parameters = Vec::new();
    let mut identity = Sha256::new();
    for (index, condition) in conditions.iter().enumerate() {
        let (expression, kind) =
            column(&condition.column).ok_or_else(|| invalid("Unknown filter column"))?;
        if condition.values.is_empty()
            || condition.values.iter().any(|v| v.len() > MAX_VALUE)
            || condition.key.len() > 256
        {
            return Err(invalid("Filter value is missing or too long"));
        }
        let single = || condition.values.first().cloned().unwrap_or_default();
        // Types and levels are stored upper-case.
        let normalized: Vec<String> = if matches!(condition.column.as_str(), "type" | "level") {
            condition.values.iter().map(|v| v.to_uppercase()).collect()
        } else {
            condition.values.clone()
        };
        let name = format!("c{index}");
        let expression = if kind == Kind::Attribute {
            if condition.key.is_empty() {
                return Err(invalid("metadata filters need a key"));
            }
            parameters.push((format!("param_{name}k"), condition.key.clone()));
            expression.replace("{key}", &format!("{{{name}k:String}}"))
        } else {
            expression.to_owned()
        };
        let numeric = kind == Kind::Number
            || (kind == Kind::Attribute
                && matches!(condition.operator.as_str(), ">" | "<" | ">=" | "<="));
        let clause = match (condition.operator.as_str(), numeric) {
            ("=", false) => {
                parameters.push((format!("param_{name}"), normalized[0].clone()));
                format!("{expression} = {{{name}:String}}")
            }
            ("=" | ">" | "<" | ">=" | "<=", true) => {
                let number: f64 = single()
                    .trim()
                    .parse()
                    .map_err(|_| invalid("Filter value must be a number"))?;
                if !number.is_finite() {
                    return Err(invalid("Filter value must be a number"));
                }
                parameters.push((format!("param_{name}"), number.to_string()));
                let expression = if kind == Kind::Attribute {
                    format!("toFloat64OrNull({expression})")
                } else {
                    expression
                };
                format!("{expression} {} {{{name}:Float64}}", condition.operator)
            }
            ("contains" | "matches", false) => {
                parameters.push((format!("param_{name}"), pattern(&single(), "%", "%")));
                format!("{expression} ILIKE {{{name}:String}}")
            }
            ("does not contain", false) => {
                parameters.push((format!("param_{name}"), pattern(&single(), "%", "%")));
                format!("{expression} NOT ILIKE {{{name}:String}}")
            }
            ("starts with", false) => {
                parameters.push((format!("param_{name}"), pattern(&single(), "", "%")));
                format!("{expression} ILIKE {{{name}:String}}")
            }
            ("ends with", false) => {
                parameters.push((format!("param_{name}"), pattern(&single(), "%", "")));
                format!("{expression} ILIKE {{{name}:String}}")
            }
            ("any of", false) => {
                parameters.push((format!("param_{name}"), array(&normalized)));
                format!("{expression} IN {{{name}:Array(String)}}")
            }
            ("none of", false) => {
                parameters.push((format!("param_{name}"), array(&normalized)));
                format!("{expression} NOT IN {{{name}:Array(String)}}")
            }
            _ => return Err(invalid("Operator does not apply to this column")),
        };
        clauses.push_str(" AND (");
        clauses.push_str(&clause);
        clauses.push_str(")\n");
        for part in [
            condition.column.as_str(),
            condition.operator.as_str(),
            condition.key.as_str(),
        ]
        .into_iter()
        .chain(normalized.iter().map(String::as_str))
        {
            identity.update(part);
            identity.update([0]);
        }
        identity.update([1]);
    }
    Ok(Rendered {
        clauses,
        parameters,
        identity: hex::encode(identity.finalize()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn condition(column: &str, operator: &str, values: &[&str], key: &str) -> pb::FilterCondition {
        pb::FilterCondition {
            column: column.into(),
            operator: operator.into(),
            values: values.iter().map(|v| v.to_string()).collect(),
            key: key.into(),
            ..Default::default()
        }
    }
    #[test]
    fn values_become_parameters_and_only_known_columns_and_operators_render() {
        let rendered = render(&[
            condition("level", "any of", &["error", "warning"], ""),
            condition("latency", ">", &["2"], ""),
            condition("metadata", "contains", &["50%"], "gen_ai.tool.name"),
        ])
        .unwrap();
        assert!(rendered.clauses.contains("Level IN {c0:Array(String)}"));
        assert!(rendered.clauses.contains("(Duration / 1e9) > {c1:Float64}"));
        assert!(
            rendered
                .clauses
                .contains("SpanAttributes[{c2k:String}] ILIKE {c2:String}")
        );
        assert_eq!(rendered.parameters[0].1, "['ERROR','WARNING']");
        assert_eq!(rendered.parameters[3].1, "%50\\%%");
        assert!(
            !rendered.clauses.contains("gen_ai"),
            "keys and values never enter the SQL text"
        );
        for bad in [
            condition("SpanName; DROP", "=", &["x"], ""),
            condition("latency", "contains", &["x"], ""),
            condition("latency", ">", &["fast"], ""),
            condition("metadata", "=", &["x"], ""),
            condition("name", "=", &[], ""),
        ] {
            assert!(render(&[bad]).is_err());
        }
    }
}
