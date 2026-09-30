//! Google Analytics (GA4): Admin API, Data API reports, audience exports/lists, report tasks
//! and Measurement Protocol events, as one table of tools (Composio's toolkit plus Nango's
//! actions). Each tool is one call: `{name}` path parameters take IDs or resource names,
//! listed query fields and the free-form `query` object become query parameters, and the
//! remaining input is the body (or more query parameters for bodiless calls). snake_case keys
//! are sent as lowerCamelCase and the response comes back with snake_case keys.
use super::Api;
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_ANALYTICS, model};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::{ToolProvider, rest};
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Map, Value, json};

const ID: &str = "google_analytics";

pub fn definition() -> model::Provider {
    super::definition(
        ID,
        "Google Analytics",
        "Google Analytics tools for GA4 Admin, Data API reports, audience exports/lists, report tasks, and Measurement Protocol events.",
        CATEGORY_ANALYTICS,
        &[
            "analytics",
            "analytics.readonly",
            "analytics.edit",
            "analytics.manage.users",
            "analytics.manage.users.readonly",
        ],
    )
}

pub struct GoogleAnalytics;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verb {
    Get,
    Post,
    Patch,
    Delete,
    /// A POST without a body, e.g. `:archive`.
    Action,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Base {
    Admin,
    Data,
    /// Measurement Protocol: authenticated by the `measurement_id`/`api_secret` query, not OAuth.
    Measurement,
}
/// Name, summary, description, verb, API, path template, query fields.
type Spec = (
    &'static str,
    &'static str,
    &'static str,
    Verb,
    Base,
    &'static str,
    &'static [&'static str],
);

impl ToolProvider for GoogleAnalytics {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        SPECS
            .iter()
            .map(|&(name, summary, description, verb, _, path, query)| {
                rest::definition(
                    ID,
                    name,
                    summary,
                    description,
                    schema(verb, path, query),
                    rest::Hints {
                        read_only: verb == Verb::Get,
                        destructive: verb == Verb::Delete,
                    },
                )
            })
            .collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let &(_, _, _, verb, base, path, query) = SPECS
                .iter()
                .find(|spec| spec.0 == name)
                .ok_or_else(|| ConnectError::not_found("Unsupported Google Analytics tool"))?;
            let (segments, query, body) = render(verb, path, query, input)?;
            let api = match base {
                Base::Admin => Api::new(
                    access,
                    "google_analytics_admin",
                    "https://analyticsadmin.googleapis.com",
                ),
                Base::Data => Api::new(
                    access,
                    "google_analytics_data",
                    "https://analyticsdata.googleapis.com",
                ),
                Base::Measurement => Api::new(
                    access,
                    "google_analytics_measurement",
                    "https://www.google-analytics.com",
                ),
            };
            let method = match verb {
                Verb::Get => Method::GET,
                Verb::Post | Verb::Action => Method::POST,
                Verb::Patch => Method::PATCH,
                Verb::Delete => Method::DELETE,
            };
            let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
            let mut request = if base == Base::Measurement {
                let url = access.url(api.endpoint, api.base, &segments)?;
                access.http.client.request(method, url)
            } else {
                api.request(method, &segments)?
            };
            request = request
                .query(&query)
                .header(reqwest::header::ACCEPT, "application/json");
            if let Some(body) = body {
                request = request.json(&body);
            }
            let (status, bytes) = rest::response(request).await?;
            let data = match rest::output(status, &bytes)?["data"].take() {
                Value::Null => json!({}),
                Value::String(text) => json!({"message": text}),
                data => data,
            };
            Ok(keys(data, snake))
        })
    }
}

type Rendered = (Vec<String>, Vec<(String, String)>, Option<Value>);

/// Path segments, query pairs and body for one call.
fn render(verb: Verb, path: &str, fields: &[&str], input: Value) -> ToolResult<Rendered> {
    let Value::Object(mut input) = input else {
        return Err(ConnectError::invalid_argument(
            "Google Analytics tool parameters must be a JSON object",
        ));
    };
    let mut segments = Vec::new();
    for template in path.trim_start_matches('/').split('/') {
        let mut segment = template.to_owned();
        for name in placeholders(template) {
            let value = input
                .remove(name)
                .and_then(|value| value.as_str().map(str::to_owned))
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ConnectError::invalid_argument(format!(
                        "missing required string parameter `{name}`"
                    ))
                })?;
            segment = segment.replace(&format!("{{{name}}}"), &id(name, &value));
        }
        // An ID may still carry slashes; each part is its own path segment.
        segments.extend(segment.split('/').map(str::to_owned));
    }
    let mut query = Vec::new();
    if let Some(extra) = input.remove("query") {
        let Value::Object(extra) = extra else {
            return Err(ConnectError::invalid_argument(
                "query must be a JSON object when provided",
            ));
        };
        for (key, value) in extra {
            pairs(&mut query, &camel(&key), value)?;
        }
    }
    for field in fields {
        if let Some(value) = input.remove(*field) {
            pairs(&mut query, &camel(field), value)?;
        }
    }
    let mut body = match input.remove("body") {
        None => Map::new(),
        Some(Value::Object(body)) => body,
        Some(_) => {
            return Err(ConnectError::invalid_argument(
                "body must be a JSON object when provided",
            ));
        }
    };
    if !has_body(verb) {
        for (key, value) in input {
            pairs(&mut query, &camel(&key), value)?;
        }
        return Ok((segments, query, None));
    }
    body.extend(input);
    Ok((
        segments,
        query,
        Some(without_nulls(keys(Value::Object(body), camel))),
    ))
}
fn has_body(verb: Verb) -> bool {
    matches!(verb, Verb::Post | Verb::Patch)
}
fn placeholders(template: &str) -> Vec<&str> {
    template
        .split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
        .collect()
}
fn pairs(query: &mut Vec<(String, String)>, key: &str, value: Value) -> ToolResult<()> {
    match value {
        Value::Null => {}
        Value::String(value) => query.push((key.to_owned(), value)),
        Value::Bool(_) | Value::Number(_) => query.push((key.to_owned(), value.to_string())),
        Value::Array(values) => {
            for value in values {
                pairs(query, key, value)?;
            }
        }
        Value::Object(_) => {
            return Err(ConnectError::invalid_argument(format!(
                "query parameter `{key}` must be scalar or array"
            )));
        }
    }
    Ok(())
}

/// The bare ID from an ID or a resource name (`properties/1/audiences/2` is `2`).
fn id(name: &str, value: &str) -> String {
    let after = |marker: &str| {
        value
            .rsplit_once(marker)
            .map_or(value, |(_, suffix)| suffix)
            .to_owned()
    };
    match name {
        "account_id" => value.strip_prefix("accounts/").unwrap_or(value).to_owned(),
        "property_id" | "property" => value
            .strip_prefix("properties/")
            .unwrap_or(value)
            .to_owned(),
        "audience_id" => after("/audiences/"),
        "audience_export_id" => after("/audienceExports/"),
        "audience_list_id" => after("/audienceLists/"),
        "conversion_event_id" => after("/conversionEvents/"),
        "custom_dimension_id" => after("/customDimensions/"),
        "data_stream_id" => after("/dataStreams/"),
        "key_event_id" => after("/keyEvents/"),
        "recurring_audience_list_id" => after("/recurringAudienceLists/"),
        "report_task_id" => after("/reportTasks/"),
        _ => value.to_owned(),
    }
}
fn describe(name: &str) -> &'static str {
    match name {
        "account_id" => {
            "Google Analytics account id or resource name, for example `12345` or `accounts/12345`."
        }
        "property_id" | "property" => {
            "GA4 property id or resource name, for example `123456789` or `properties/123456789`."
        }
        "data_stream_id" => "Data stream id or resource name.",
        "audience_id" => "Audience id or resource name.",
        "audience_export_id" => "Audience export id or resource name.",
        "audience_list_id" => "Audience list id or resource name.",
        "recurring_audience_list_id" => "Recurring audience list id or resource name.",
        "report_task_id" => "Report task id or resource name.",
        "custom_dimension_id" => "Custom dimension id or resource name.",
        "conversion_event_id" => "Conversion event id or resource name.",
        "key_event_id" => "Key event id or resource name.",
        _ => "",
    }
}

fn schema(verb: Verb, path: &str, fields: &[&str]) -> Value {
    let scalar = json!({"type":["string","number","boolean","array"],"items":{"type":"string"}});
    let mut properties = Map::new();
    let names = placeholders(path);
    for name in &names {
        properties.insert(
            (*name).to_owned(),
            json!({"type":"string","description":describe(name)}),
        );
    }
    for field in fields {
        let mut property = scalar.clone();
        property["description"] = json!(format!(
            "Optional Google Analytics query parameter `{}`.",
            camel(field)
        ));
        properties.insert((*field).to_owned(), property);
    }
    properties.insert(
        "query".into(),
        json!({
            "type":"object",
            "additionalProperties":scalar,
            "description":"Additional Google Analytics query parameters. snake_case keys are converted to lowerCamelCase."
        }),
    );
    if has_body(verb) {
        properties.insert(
            "body".into(),
            json!({
                "type":"object",
                "additionalProperties":true,
                "description":"Google Analytics request body. snake_case keys are converted to lowerCamelCase. Top-level fields not used as path or query parameters are also merged into this body."
            }),
        );
    }
    json!({"type":"object","properties":properties,"required":names,"additionalProperties":has_body(verb)})
}

fn keys(value: Value, rename: fn(&str) -> String) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (rename(&key), keys(value, rename)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(|v| keys(v, rename)).collect()),
        scalar => scalar,
    }
}
fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, without_nulls(value)))
                .filter(|(_, value)| !value.is_null())
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(without_nulls).collect()),
        scalar => scalar,
    }
}
fn camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for c in key.chars() {
        match c {
            '_' => upper = true,
            c if upper => {
                out.extend(c.to_uppercase());
                upper = false;
            }
            c => out.push(c),
        }
    }
    out
}
fn snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (i, c) in key.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

const NONE: &[&str] = &[];
const PAGE: &[&str] = &["page_size", "page_token"];
const PAGE_FILTER: &[&str] = &["page_size", "page_token", "filter"];
const UPDATE_MASK: &[&str] = &["update_mask"];
const MP: &[&str] = &["measurement_id", "api_secret"];

#[rustfmt::skip]
const SPECS: &[Spec] = {
    use Base::{Admin, Data, Measurement};
    use Verb::{Action, Delete, Get, Patch, Post};
    &[
        ("google_analytics_archive_conversion_event", "Archive Conversion Event", "Archive a deprecated GA4 conversion event.", Delete, Admin, "/v1beta/properties/{property_id}/conversionEvents/{conversion_event_id}", NONE),
        ("google_analytics_archive_custom_dimension", "Archive Custom Dimension", "Archive a GA4 custom dimension.", Action, Admin, "/v1beta/properties/{property_id}/customDimensions/{custom_dimension_id}:archive", NONE),
        ("google_analytics_batch_run_pivot_reports", "Batch Run Pivot Reports", "Run multiple GA4 pivot reports in one request.", Post, Data, "/v1beta/properties/{property}:batchRunPivotReports", NONE),
        ("google_analytics_batch_run_reports", "Batch Run Reports", "Run multiple GA4 reports in one request.", Post, Data, "/v1beta/properties/{property}:batchRunReports", NONE),
        ("google_analytics_check_compatibility", "Check Compatibility", "Check GA4 dimension and metric compatibility.", Post, Data, "/v1beta/properties/{property}:checkCompatibility", NONE),
        ("google_analytics_create_audience_export", "Create Audience Export", "Create a GA4 audience export.", Post, Data, "/v1alpha/properties/{property_id}/audienceExports", NONE),
        ("google_analytics_create_audience_list", "Create Audience List", "Create a GA4 audience list.", Post, Data, "/v1alpha/properties/{property_id}/audienceLists", NONE),
        ("google_analytics_create_conversion_event", "Create Conversion Event", "Create a deprecated GA4 conversion event.", Post, Admin, "/v1beta/properties/{property_id}/conversionEvents", NONE),
        ("google_analytics_create_custom_dimension", "Create Custom Dimension", "Create a GA4 custom dimension.", Post, Admin, "/v1beta/properties/{property_id}/customDimensions", NONE),
        ("google_analytics_create_custom_metric", "Create Custom Metric", "Create a GA4 custom metric.", Post, Admin, "/v1beta/properties/{property_id}/customMetrics", NONE),
        ("google_analytics_create_data_stream", "Create Data Stream", "Create a data stream for a GA4 property.", Post, Admin, "/v1beta/properties/{property_id}/dataStreams", NONE),
        ("google_analytics_create_expanded_data_set", "Create Expanded Data Set", "Create an expanded data set for a GA4 property.", Post, Admin, "/v1alpha/properties/{property_id}/expandedDataSets", NONE),
        ("google_analytics_create_property", "Create Property", "Create a GA4 property under an account.", Post, Admin, "/v1beta/properties", NONE),
        ("google_analytics_create_recurring_audience_list", "Create Recurring Audience List", "Create a recurring audience list.", Post, Data, "/v1alpha/properties/{property_id}/recurringAudienceLists", NONE),
        ("google_analytics_create_report_task", "Create Report Task", "Create an asynchronous GA4 report task.", Post, Data, "/v1alpha/properties/{property_id}/reportTasks", NONE),
        ("google_analytics_create_rollup_property", "Create Rollup Property", "Create a GA4 roll-up property.", Post, Admin, "/v1alpha/properties:createRollupProperty", NONE),
        ("google_analytics_get_account", "Get Account", "Retrieve a Google Analytics account.", Get, Admin, "/v1beta/accounts/{account_id}", NONE),
        ("google_analytics_get_attribution_settings", "Get Attribution Settings", "Retrieve attribution settings for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/attributionSettings", NONE),
        ("google_analytics_get_audience", "Get Audience", "Retrieve a GA4 audience.", Get, Admin, "/v1alpha/properties/{property_id}/audiences/{audience_id}", NONE),
        ("google_analytics_get_audience_export", "Get Audience Export", "Retrieve a GA4 audience export.", Get, Data, "/v1alpha/properties/{property_id}/audienceExports/{audience_export_id}", NONE),
        ("google_analytics_get_audience_list", "Get Audience List", "Retrieve a GA4 audience list.", Get, Data, "/v1alpha/properties/{property_id}/audienceLists/{audience_list_id}", NONE),
        ("google_analytics_get_custom_dimension", "Get Custom Dimension", "Retrieve a GA4 custom dimension.", Get, Admin, "/v1beta/properties/{property_id}/customDimensions/{custom_dimension_id}", NONE),
        ("google_analytics_get_data_retention_settings", "Get Data Retention Settings", "Retrieve data retention settings for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/dataRetentionSettings", NONE),
        ("google_analytics_get_data_sharing_settings", "Get Data Sharing Settings", "Retrieve account data sharing settings.", Get, Admin, "/v1beta/accounts/{account_id}/dataSharingSettings", NONE),
        ("google_analytics_get_google_signals_settings", "Get Google Signals Settings", "Retrieve Google signals settings for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/googleSignalsSettings", NONE),
        ("google_analytics_get_key_event", "Get Key Event", "Retrieve a GA4 key event.", Get, Admin, "/v1beta/properties/{property_id}/keyEvents/{key_event_id}", NONE),
        ("google_analytics_get_metadata", "Get Metadata", "Retrieve GA4 metadata for a property. Use property `0` for universal metadata.", Get, Data, "/v1beta/properties/{property}/metadata", NONE),
        ("google_analytics_get_property", "Get Property", "Retrieve a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}", NONE),
        ("google_analytics_get_property_quotas_snapshot", "Get Property Quotas Snapshot", "Retrieve GA4 property quota snapshots.", Get, Data, "/v1alpha/properties/{property_id}/propertyQuotasSnapshot", NONE),
        ("google_analytics_get_recurring_audience_list", "Get Recurring Audience List", "Retrieve a recurring audience list.", Get, Data, "/v1alpha/properties/{property_id}/recurringAudienceLists/{recurring_audience_list_id}", NONE),
        ("google_analytics_get_report_task", "Get Report Task", "Retrieve a report task.", Get, Data, "/v1alpha/properties/{property_id}/reportTasks/{report_task_id}", NONE),
        ("google_analytics_list_accounts", "List Accounts (Deprecated)", "List Google Analytics accounts using the v1alpha endpoint.", Get, Admin, "/v1alpha/accounts", PAGE),
        ("google_analytics_list_account_summaries", "List Account Summaries", "List Google Analytics account summaries.", Get, Admin, "/v1beta/accountSummaries", PAGE),
        ("google_analytics_list_accounts_v1_beta", "List Accounts (v1beta)", "List Google Analytics accounts using the v1beta endpoint.", Get, Admin, "/v1beta/accounts", &["page_size", "page_token", "show_deleted"]),
        ("google_analytics_list_adsense_links", "List AdSense Links", "List AdSense links for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/adSenseLinks", PAGE),
        ("google_analytics_list_audience_exports", "List Audience Exports", "List audience exports for a GA4 property.", Get, Data, "/v1alpha/properties/{property_id}/audienceExports", PAGE),
        ("google_analytics_list_audience_lists", "List Audience Lists", "List audience lists for a GA4 property.", Get, Data, "/v1alpha/properties/{property_id}/audienceLists", PAGE),
        ("google_analytics_list_audiences", "List Audiences", "List audiences for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/audiences", PAGE),
        ("google_analytics_list_bigquery_links", "List BigQuery Links", "List BigQuery links for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/bigQueryLinks", PAGE),
        ("google_analytics_list_calculated_metrics", "List Calculated Metrics", "List calculated metrics for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/calculatedMetrics", PAGE),
        ("google_analytics_list_channel_groups", "List Channel Groups", "List channel groups for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/channelGroups", PAGE),
        ("google_analytics_list_conversion_events", "List Conversion Events", "List deprecated GA4 conversion events.", Get, Admin, "/v1beta/properties/{property_id}/conversionEvents", PAGE),
        ("google_analytics_list_custom_dimensions", "List Custom Dimensions", "List custom dimensions for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/customDimensions", PAGE),
        ("google_analytics_list_custom_metrics", "List Custom Metrics", "List custom metrics for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/customMetrics", PAGE),
        ("google_analytics_list_data_streams", "List DataStreams", "List data streams for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/dataStreams", PAGE),
        ("google_analytics_list_dv360_ad_links", "List Display & Video 360 Advertiser Links", "List Display & Video 360 advertiser links.", Get, Admin, "/v1alpha/properties/{property_id}/displayVideo360AdvertiserLinks", PAGE),
        ("google_analytics_list_dv360_link_proposals", "List DisplayVideo360 Advertiser Link Proposals", "List Display & Video 360 advertiser link proposals.", Get, Admin, "/v1alpha/properties/{property_id}/displayVideo360AdvertiserLinkProposals", PAGE),
        ("google_analytics_list_event_create_rules", "List Event Create Rules", "List event create rules for a data stream.", Get, Admin, "/v1alpha/properties/{property_id}/dataStreams/{data_stream_id}/eventCreateRules", PAGE),
        ("google_analytics_list_expanded_data_sets", "List Expanded Data Sets", "List expanded data sets for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/expandedDataSets", PAGE),
        ("google_analytics_list_firebase_links", "List Firebase Links", "List Firebase links for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/firebaseLinks", PAGE),
        ("google_analytics_list_google_ads_links", "List Google Ads Links", "List Google Ads links for a GA4 property.", Get, Admin, "/v1beta/properties/{property_id}/googleAdsLinks", PAGE),
        ("google_analytics_list_key_events", "List Key Events", "List GA4 key events.", Get, Admin, "/v1beta/properties/{property_id}/keyEvents", PAGE),
        ("google_analytics_list_measurement_protocol_secrets", "List Measurement Protocol Secrets", "List Measurement Protocol secrets for a data stream.", Get, Admin, "/v1beta/properties/{property_id}/dataStreams/{data_stream_id}/measurementProtocolSecrets", PAGE),
        ("google_analytics_list_properties", "List Properties (Deprecated)", "List GA4 properties using the v1alpha endpoint.", Get, Admin, "/v1alpha/properties", PAGE_FILTER),
        ("google_analytics_list_properties_filtered", "List Properties", "List GA4 properties using filter criteria.", Get, Admin, "/v1beta/properties", PAGE_FILTER),
        ("google_analytics_list_recurring_audience_lists", "List Recurring Audience Lists", "List recurring audience lists for a GA4 property.", Get, Data, "/v1alpha/properties/{property_id}/recurringAudienceLists", PAGE),
        ("google_analytics_list_reporting_data_annotations", "List Reporting Data Annotations", "List reporting data annotations for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/reportingDataAnnotations", PAGE),
        ("google_analytics_list_report_tasks", "List Report Tasks", "List report tasks for a GA4 property.", Get, Data, "/v1alpha/properties/{property_id}/reportTasks", PAGE),
        ("google_analytics_list_search_ads360_links", "List Search Ads 360 Links", "List Search Ads 360 links for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/searchAds360Links", PAGE),
        ("google_analytics_list_sk_ad_network_conversion_value_schemas", "List SKAdNetwork Conversion Value Schemas", "Retrieve SKAdNetwork conversion value schema configuration for a data stream.", Get, Admin, "/v1alpha/properties/{property_id}/dataStreams/{data_stream_id}/sKAdNetworkConversionValueSchema", NONE),
        ("google_analytics_list_subproperty_event_filters", "List Subproperty Event Filters", "List subproperty event filters for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/subpropertyEventFilters", PAGE),
        ("google_analytics_list_subproperty_sync_configs", "List Subproperty Sync Configs", "List subproperty sync configs for a GA4 property.", Get, Admin, "/v1alpha/properties/{property_id}/subpropertySyncConfigs", PAGE),
        ("google_analytics_provision_account_ticket", "Provision Account Ticket", "Provision a Google Analytics account ticket.", Post, Admin, "/v1alpha/accounts:provisionAccountTicket", NONE),
        ("google_analytics_query_audience_export", "Query Audience Export", "Query rows from a completed audience export.", Post, Data, "/v1alpha/properties/{property_id}/audienceExports/{audience_export_id}:query", NONE),
        ("google_analytics_query_audience_list", "Query Audience List", "Query rows from a completed audience list.", Post, Data, "/v1alpha/properties/{property_id}/audienceLists/{audience_list_id}:query", NONE),
        ("google_analytics_query_report_task", "Query Report Task", "Query rows from a completed report task.", Post, Data, "/v1alpha/properties/{property_id}/reportTasks/{report_task_id}:query", NONE),
        ("google_analytics_run_funnel_report", "Run Funnel Report", "Run a GA4 funnel report.", Post, Data, "/v1alpha/properties/{property}:runFunnelReport", NONE),
        ("google_analytics_run_pivot_report", "Run Pivot Report", "Run a GA4 pivot report.", Post, Data, "/v1beta/properties/{property}:runPivotReport", NONE),
        ("google_analytics_run_realtime_report", "Run Realtime Report", "Run a realtime GA4 report.", Post, Data, "/v1beta/properties/{property}:runRealtimeReport", NONE),
        ("google_analytics_run_report", "Run Report", "Run a GA4 report.", Post, Data, "/v1beta/properties/{property}:runReport", NONE),
        ("google_analytics_send_events", "Send Events", "Send Measurement Protocol events to Google Analytics.", Post, Measurement, "/mp/collect", MP),
        ("google_analytics_update_property", "Update Property", "Update a GA4 property.", Patch, Admin, "/v1beta/properties/{property_id}", UPDATE_MASK),
        ("google_analytics_update_data_stream", "Update Data Stream", "Update a GA4 data stream.", Patch, Admin, "/v1beta/properties/{property_id}/dataStreams/{data_stream_id}", UPDATE_MASK),
        ("google_analytics_validate_events", "Validate Events", "Validate Measurement Protocol events with the debug endpoint.", Post, Measurement, "/debug/mp/collect", MP),
    ]
};
