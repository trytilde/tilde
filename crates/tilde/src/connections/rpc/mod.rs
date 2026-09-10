use crate::proto::tilde::setup::v1 as setup_pb;
use crate::proto::tilde::types::v1 as types;
use secrecy::SecretString;
pub mod management;
pub mod setup;
use crate::{connections::model as m, error::Error};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(super) fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| m::invalid("Invalid UUID"))
}
pub(super) fn driver_number(driver: m::Driver) -> i32 {
    match driver {
        m::Driver::Static => 1,
        m::Driver::OAuthCode => 2,
        m::Driver::OAuthClientCredentials => 3,
        m::Driver::OAuthJwtBearer => 4,
        m::Driver::Custom => 5,
    }
}
pub(super) fn provider_wire(provider: m::Provider) -> types::Provider {
    types::Provider {
        id: provider.id,
        name: provider.name,
        kind: Some(match provider.kind {
            m::ProviderKind::BuiltIn => types::provider::Kind::BuiltIn(Box::default()),
            m::ProviderKind::Configured => types::provider::Kind::Configured(Box::default()),
            m::ProviderKind::Remote(remote) => {
                types::provider::Kind::Remote(Box::new(types::RemoteProvider {
                    endpoint: remote.endpoint,
                    ui_url: remote.ui_url,
                    ..Default::default()
                }))
            }
        }),
        categories: provider.categories,
        connection_types: provider
            .connection_types
            .into_iter()
            .map(connection_type_wire)
            .collect(),
        ..Default::default()
    }
}
pub(super) fn connection_type_wire(typ: m::ConnectionType) -> types::ConnectionType {
    use types::connection_type::CredentialSource as Wire;
    let source = match typ.credential_source {
        m::CredentialSource::Static { schema } => {
            Wire::Static(Box::new(types::StaticCredentialSource {
                schema_json: schema.to_string(),
                ..Default::default()
            }))
        }
        m::CredentialSource::OAuth {
            grant,
            configuration,
            additional_schema,
        } => Wire::Oauth(Box::new(types::OAuthCredentialSource {
            grant: (match grant {
                m::OAuthGrant::AuthorizationCode => 1,
                m::OAuthGrant::ClientCredentials => 2,
                m::OAuthGrant::JwtBearer => 3,
            })
            .into(),
            configuration: oauth_wire(*configuration).into(),
            additional_schema_json: additional_schema.map(|schema| schema.to_string()),
            ..Default::default()
        })),
        m::CredentialSource::Custom => Wire::Custom(Box::default()),
    };
    types::ConnectionType {
        id: typ.id,
        name: typ.name,
        capabilities: typ
            .capabilities
            .into_iter()
            .map(|_| types::Capability::Channel.into())
            .collect(),
        credential_source: Some(source),
        ..Default::default()
    }
}
pub(super) fn oauth_wire(o: m::OAuth) -> types::OAuthConfiguration {
    types::OAuthConfiguration {
        authorization_url: o.authorization_url,
        token_url: o.token_url,
        client_authentication: (match o.client_auth {
            m::ClientAuth::Body => 1,
            m::ClientAuth::Basic => 2,
            m::ClientAuth::None => 3,
        })
        .into(),
        pkce: o.pkce,
        scopes: o.scopes,
        scope_separator: o.scope_separator,
        authorization_parameters: parameters_wire(o.authorization_parameters),
        token_parameters: parameters_wire(o.token_parameters),
        access_token_path: o.access_token_path,
        refresh_token_path: o.refresh_token_path,
        expires_in_path: o.expires_in_path,
        scope_path: o.scope_path,
        success_path: o.success_path,
        result_fields: o
            .result_fields
            .into_iter()
            .map(|f| types::OAuthResultField {
                key: f.key,
                path: f.path,
                required: f.required,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}
pub(super) fn oauth_model(o: types::OAuthConfiguration) -> Result<m::OAuth, Error> {
    Ok(m::OAuth {
        authorization_url: o.authorization_url,
        token_url: o.token_url,
        client_auth: match o.client_authentication.to_i32() {
            0 | 1 => m::ClientAuth::Body,
            2 => m::ClientAuth::Basic,
            3 => m::ClientAuth::None,
            _ => return Err(m::invalid("Unknown OAuth client authentication")),
        },
        pkce: o.pkce,
        scopes: o.scopes,
        scope_separator: if o.scope_separator.is_empty() {
            " ".into()
        } else {
            o.scope_separator
        },
        authorization_parameters: parameters_model(o.authorization_parameters)?,
        token_parameters: parameters_model(o.token_parameters)?,
        access_token_path: if o.access_token_path.is_empty() {
            "/access_token".into()
        } else {
            o.access_token_path
        },
        refresh_token_path: if o.refresh_token_path.is_empty() {
            "/refresh_token".into()
        } else {
            o.refresh_token_path
        },
        expires_in_path: if o.expires_in_path.is_empty() {
            "/expires_in".into()
        } else {
            o.expires_in_path
        },
        scope_path: if o.scope_path.is_empty() {
            "/scope".into()
        } else {
            o.scope_path
        },
        success_path: o.success_path,
        result_fields: o
            .result_fields
            .into_iter()
            .map(|field| m::ResultField {
                key: field.key,
                path: field.path,
                required: field.required,
            })
            .collect(),
    })
}
pub(super) fn parameters_wire(values: BTreeMap<String, String>) -> Vec<types::Parameter> {
    values
        .into_iter()
        .map(|(name, value)| types::Parameter {
            name,
            value,
            ..Default::default()
        })
        .collect()
}
pub(super) fn parameters_model(
    values: Vec<types::Parameter>,
) -> Result<BTreeMap<String, String>, Error> {
    let mut out = BTreeMap::new();
    for value in values {
        if out.insert(value.name, value.value).is_some() {
            return Err(m::invalid("Duplicate OAuth parameter"));
        }
    }
    Ok(out)
}
pub(super) fn provider_model(provider: types::Provider) -> Result<m::Provider, Error> {
    let mut types = vec![];
    for typ in provider.connection_types {
        use types::connection_type::CredentialSource as Wire;
        let credential_source = match typ.credential_source {
            Some(Wire::Static(source)) => m::CredentialSource::Static {
                schema: serde_json::from_str(&source.schema_json)
                    .map_err(|_| m::invalid("Invalid credential schema JSON"))?,
            },
            Some(Wire::Oauth(source)) => m::CredentialSource::OAuth {
                additional_schema: source
                    .additional_schema_json
                    .map(|schema| {
                        serde_json::from_str(&schema)
                            .map_err(|_| m::invalid("Invalid OAuth credential schema JSON"))
                    })
                    .transpose()?,
                grant: match source.grant.to_i32() {
                    1 => m::OAuthGrant::AuthorizationCode,
                    2 => m::OAuthGrant::ClientCredentials,
                    3 => m::OAuthGrant::JwtBearer,
                    _ => return Err(m::invalid("OAuth grant required")),
                },
                configuration: oauth_model(
                    source
                        .configuration
                        .into_option()
                        .ok_or_else(|| m::invalid("OAuth configuration required"))?,
                )?
                .into(),
            },
            Some(Wire::Custom(_)) => m::CredentialSource::Custom,
            None => return Err(m::invalid("Credential source required")),
        };
        let capabilities = typ
            .capabilities
            .into_iter()
            .map(|cap| {
                if cap == types::Capability::Channel {
                    Ok(m::Capability::Channel)
                } else {
                    Err(m::invalid("Unknown capability"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        types.push(m::ConnectionType {
            id: typ.id,
            name: typ.name,
            capabilities,
            credential_source,
        });
    }
    Ok(m::Provider {
        id: provider.id,
        name: provider.name,
        kind: match provider.kind {
            Some(types::provider::Kind::BuiltIn(_)) => m::ProviderKind::BuiltIn,
            Some(types::provider::Kind::Configured(_)) => m::ProviderKind::Configured,
            Some(types::provider::Kind::Remote(remote)) => {
                m::ProviderKind::Remote(m::RemoteProvider {
                    endpoint: remote.endpoint,
                    ui_url: remote.ui_url,
                })
            }
            None => return Err(m::invalid("Provider kind is required")),
        },
        connection_types: types,
        categories: provider.categories,
    })
}
pub(super) fn connection_wire(c: m::Connection, base: &str) -> types::Connection {
    let timestamp =
        |date: chrono::DateTime<chrono::Utc>| buffa_types::google::protobuf::Timestamp {
            seconds: date.timestamp(),
            nanos: date.timestamp_subsec_nanos() as i32,
            ..Default::default()
        };
    types::Connection {
        webhook_url: c
            .channel_capable
            .then(|| format!("{base}/connections/webhooks/{}", c.id)),
        id: c.id.to_string(),
        name: c.name,
        provider_id: c.provider_id,
        type_id: c.type_id,
        status: c.status,
        account_label: c.account_label,
        associated_agents: c
            .associated_agents
            .0
            .into_iter()
            .map(|a| types::AssociatedAgent {
                capability: types::Capability::Channel.into(),
                id: a.id.to_string(),
                name: a.name,
                endpoint_url: a.endpoint_url,
                ..Default::default()
            })
            .collect(),
        capabilities: if c.channel_capable {
            vec![types::Capability::Channel.into()]
        } else {
            vec![]
        },
        created_at: timestamp(c.created_at).into(),
        updated_at: timestamp(c.updated_at).into(),
        ..Default::default()
    }
}
pub(super) fn broker_wire(view: m::BrokerView) -> setup_pb::Brokering {
    use setup_pb::brokering::Action;
    let action = match view.action {
        m::Action::Form => Action::Form(Box::default()),
        m::Action::Redirect { url } => Action::Redirect(Box::new(setup_pb::RedirectAction {
            url,
            ..Default::default()
        })),
        m::Action::FormPost { url, fields } => {
            Action::FormPost(Box::new(setup_pb::FormPostAction {
                url,
                fields: fields
                    .into_iter()
                    .map(|(name, value)| types::Parameter {
                        name,
                        value,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }))
        }
        m::Action::Complete => Action::Complete(Box::default()),
        m::Action::Working => Action::Working(Box::default()),
        m::Action::Failed => Action::Failed(Box::default()),
        m::Action::Cancelled => Action::Cancelled(Box::default()),
    };
    setup_pb::Brokering {
        webhook_url: view.webhook_url,
        setup_id: view.setup_id.to_string(),
        connection_id: view.connection_id.to_string(),
        connection_name: view.connection_name,
        type_name: view.type_name,
        type_id: view.type_id,
        action_id: view.action_id.to_string(),
        step: view.step,
        error_code: view.error_code,
        ui_path: view.ui_path,
        auth_driver: driver_number(view.auth_driver).into(),
        input_schema_json: view.input_schema.map(|schema| schema.to_string()),
        draft: view
            .draft
            .into_iter()
            .map(|(key, value)| {
                use secrecy::ExposeSecret;
                types::InputField {
                    key,
                    value: value.expose_secret().into(),
                    ..Default::default()
                }
            })
            .collect(),
        action: Some(action),
        ..Default::default()
    }
}

pub(super) fn input_values(fields: Vec<types::InputField>) -> Result<m::Values, Error> {
    let mut values = m::Values::new();
    for field in fields {
        if values
            .insert(field.key, SecretString::from(field.value))
            .is_some()
        {
            return Err(m::invalid("Duplicate input field"));
        }
    }
    super::setup::bounded(&values)?;
    Ok(values)
}

pub(super) fn assignment_model(a: types::CapabilityAssignment) -> Result<m::Assignment, Error> {
    if a.capability != types::Capability::Channel {
        return Err(m::invalid("Unsupported capability"));
    }
    Ok(m::Assignment {
        capability: m::Capability::Channel,
        agent_id: id(&a.agent_id)?,
    })
}
