//! Curated remote MCP servers, carried over from trytilde/api's catalogue. Each server is a
//! built-in provider (or joins the built-in provider of the same service, such as Slack) whose
//! connection types are the ways to reach it; every type names the server as its tool source, so
//! its tools are discovered from the server once a connection is ready.
use crate::connections::model::{
    Capability, ClientAuth, ConnectionType, CredentialSource, McpCredential, McpServer, OAuth,
    OAuthClient, OAuthGrant, Provider, ProviderKind,
};
use serde_json::json;

/// How Tilde reaches a catalogue server.
enum Access {
    /// Public server: no credentials.
    Public,
    /// OAuth with a client registered dynamically at setup.
    Dynamic,
    /// OAuth with the person's own app registered at the provider.
    App {
        authorize: &'static str,
        token: &'static str,
        scopes: &'static [&'static str],
        client_auth: ClientAuth,
    },
    /// An API key sent in a header, as a query parameter, or as a bearer token.
    Header(&'static str),
    Query(&'static str),
    Bearer,
}
struct Server {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    url: &'static str,
    categories: &'static [&'static str],
    access: Access,
}

const SLACK_SCOPES: &[&str] = &[
    "search:read.public",
    "search:read.private",
    "search:read.mpim",
    "search:read.im",
    "search:read.files",
    "files:read",
    "emoji:read",
    "search:read.users",
    "chat:write",
    "channels:history",
    "groups:history",
    "mpim:history",
    "im:history",
    "channels:write",
    "groups:write",
    "im:write",
    "mpim:write",
    "reactions:write",
    "canvases:read",
    "canvases:write",
    "users:read",
    "users:read.email",
    "channels:read",
    "groups:read",
    "mpim:read",
];

const SERVERS: &[Server] = &[
    Server {
        id: "agentmail",
        name: "AgentMail",
        description: "Manage AgentMail inboxes, email threads, messages, drafts, and attachments through AgentMail's hosted MCP server.",
        icon: "agentmail.png",
        url: "https://mcp.agentmail.to/mcp",
        categories: &["inbox_and_collaboration"],
        access: Access::Header("x-api-key"),
    },
    Server {
        id: "notion",
        name: "Notion",
        description: "Search, read, and update Notion workspaces through Notion's official remote MCP server.",
        icon: "notion.svg",
        url: "https://mcp.notion.com/mcp",
        categories: &["documents_and_files"],
        access: Access::Dynamic,
    },
    Server {
        id: "granola",
        name: "Granola",
        description: "Query meeting notes, transcripts, folders, and account information through Granola's official MCP server.",
        icon: "granola.svg",
        url: "https://mcp.granola.ai/mcp",
        categories: &["documents_and_files"],
        access: Access::Dynamic,
    },
    Server {
        id: "browserbase",
        name: "Browserbase",
        description: "Run browser sessions, navigate pages, act on elements, observe controls, and extract data through Browserbase's official MCP server.",
        icon: "browserbase.svg",
        url: "https://mcp.browserbase.com/mcp",
        categories: &["browser_automation"],
        access: Access::Query("browserbaseApiKey"),
    },
    Server {
        id: "intercom",
        name: "Intercom",
        description: "Work with Intercom conversations and customer-support data through Intercom's official MCP server.",
        icon: "intercom.svg",
        url: "https://mcp.intercom.com/mcp",
        categories: &["customer_support"],
        access: Access::Dynamic,
    },
    Server {
        id: "parallel-search",
        name: "Parallel Search",
        description: "Search the live web and fetch pages through Parallel's official Search MCP server.",
        icon: "parallel.svg",
        url: "https://search.parallel.ai/mcp",
        categories: &["search"],
        access: Access::Public,
    },
    Server {
        id: "parallel-task",
        name: "Parallel Task",
        description: "Run asynchronous deep research and batch enrichment through Parallel's official Task MCP server.",
        icon: "parallel.svg",
        url: "https://task-mcp.parallel.ai/mcp",
        categories: &["search"],
        access: Access::Bearer,
    },
    Server {
        id: "slack",
        name: "Slack",
        description: "Search and act on Slack workspace content through Slack's hosted MCP server.",
        icon: "slack.svg",
        url: "https://mcp.slack.com/mcp",
        categories: &["inbox_and_collaboration"],
        access: Access::App {
            authorize: "https://slack.com/oauth/v2_user/authorize",
            token: "https://slack.com/api/oauth.v2.user.access",
            scopes: SLACK_SCOPES,
            client_auth: ClientAuth::Body,
        },
    },
    Server {
        id: "linear",
        name: "Linear",
        description: "Manage Linear issues, projects, and team workflows through Linear's hosted MCP server.",
        icon: "linear.svg",
        url: "https://mcp.linear.app/mcp",
        categories: &["productivity"],
        access: Access::Dynamic,
    },
    Server {
        id: "zoom",
        name: "Zoom",
        description: "Access Zoom meetings and collaboration workflows through Zoom's hosted MCP server.",
        icon: "zoom.svg",
        url: "https://mcp.zoom.us/mcp/zoom/streamable",
        categories: &["inbox_and_collaboration"],
        access: Access::App {
            authorize: "https://zoom.us/oauth/authorize",
            token: "https://zoom.us/oauth/token",
            scopes: &[],
            client_auth: ClientAuth::Basic,
        },
    },
    Server {
        id: "salesforce",
        name: "Salesforce",
        description: "Access Salesforce platform objects through Salesforce's hosted platform MCP endpoint.",
        icon: "salesforce.svg",
        url: "https://api.salesforce.com/platform/mcp/v1/platform/sobject-all",
        categories: &["sales"],
        access: Access::App {
            authorize: "https://login.salesforce.com/services/oauth2/authorize",
            token: "https://login.salesforce.com/services/oauth2/token",
            scopes: &["api", "refresh_token"],
            client_auth: ClientAuth::Body,
        },
    },
    Server {
        id: "clay",
        name: "Clay",
        description: "Use Clay enrichment and go-to-market workflows through Clay's hosted MCP endpoint.",
        icon: "clay.png",
        url: "https://api.clay.com/v3/mcp",
        categories: &["sales"],
        access: Access::Dynamic,
    },
    Server {
        id: "apollo",
        name: "Apollo.io",
        description: "Search and enrich sales intelligence through Apollo's hosted MCP server.",
        icon: "apollo.svg",
        url: "https://mcp.apollo.io/mcp",
        categories: &["sales"],
        access: Access::Dynamic,
    },
    Server {
        id: "hubspot",
        name: "HubSpot",
        description: "Work with HubSpot CRM and marketing data through HubSpot's hosted MCP server.",
        icon: "hubspot.svg",
        url: "https://mcp.hubspot.com",
        categories: &["sales"],
        access: Access::App {
            authorize: "https://mcp.hubspot.com/oauth/authorize/user",
            token: "https://mcp.hubspot.com/oauth/v3/token",
            scopes: &[],
            client_auth: ClientAuth::Body,
        },
    },
    Server {
        id: "stripe",
        name: "Stripe",
        description: "Inspect and manage Stripe payment resources through Stripe's hosted MCP server.",
        icon: "stripe.svg",
        url: "https://mcp.stripe.com",
        categories: &["payments"],
        access: Access::Dynamic,
    },
    Server {
        id: "circle",
        name: "Circle",
        description: "Use Circle's payment and stablecoin APIs through its hosted MCP endpoint.",
        icon: "circle.svg",
        url: "https://api.circle.com/v1/codegen/mcp",
        categories: &["payments"],
        access: Access::Public,
    },
    Server {
        id: "vercel",
        name: "Vercel",
        description: "Inspect and manage Vercel projects and deployments through Vercel's hosted MCP server.",
        icon: "vercel.svg",
        url: "https://mcp.vercel.com",
        categories: &["infrastructure"],
        access: Access::Dynamic,
    },
    Server {
        id: "neon",
        name: "Neon",
        description: "Manage Neon Postgres projects and databases through Neon's hosted MCP server.",
        icon: "neon.svg",
        url: "https://mcp.neon.tech/mcp",
        categories: &["infrastructure", "data_analytics"],
        access: Access::Dynamic,
    },
    Server {
        id: "cloudflare-docs",
        name: "Cloudflare Documentation",
        description: "Search Cloudflare developer documentation through Cloudflare's public MCP server.",
        icon: "cloudflare.svg",
        url: "https://docs.mcp.cloudflare.com/mcp",
        categories: &["documentation"],
        access: Access::Public,
    },
    Server {
        id: "aws-knowledge",
        name: "AWS Knowledge",
        description: "Search AWS documentation and knowledge through AWS's public remote MCP server.",
        icon: "aws.svg",
        url: "https://knowledge-mcp.global.api.aws",
        categories: &["documentation"],
        access: Access::Public,
    },
    Server {
        id: "mintlify-docs",
        name: "Mintlify Documentation",
        description: "Search Mintlify documentation through its public MCP endpoint.",
        icon: "mintlify.svg",
        url: "https://www.mintlify.com/docs/mcp",
        categories: &["documentation"],
        access: Access::Public,
    },
    Server {
        id: "svelte-docs",
        name: "Svelte Documentation",
        description: "Search Svelte documentation through the Svelte project's public MCP server.",
        icon: "svelte.svg",
        url: "https://mcp.svelte.dev/mcp",
        categories: &["documentation"],
        access: Access::Public,
    },
    Server {
        id: "twilio-docs",
        name: "Twilio Documentation",
        description: "Search Twilio developer documentation through Twilio's public MCP server.",
        icon: "twilio.svg",
        url: "https://mcp.twilio.com/docs",
        categories: &["documentation"],
        access: Access::Public,
    },
];

fn api_key(title: &str) -> CredentialSource {
    CredentialSource::Static {
        schema: json!({"type":"object","properties":{"api_key":{"type":"string","title":title,"minLength":1,"writeOnly":true}},"required":["api_key"],"additionalProperties":false}),
    }
}
/// The connection type reaching `server`. `prefix` keeps its ID apart from the types of a
/// built-in provider it joins.
fn connection_type(server: &Server, prefix: &str) -> ConnectionType {
    let mcp = |credential| {
        Some(McpServer {
            url: server.url.into(),
            credential,
        })
    };
    let oauth = |configuration: OAuth| CredentialSource::OAuth {
        grant: OAuthGrant::AuthorizationCode,
        configuration: configuration.into(),
        additional_schema: None,
    };
    let (id, name, credential_source, mcp) = match &server.access {
        Access::Public => (
            "public",
            "No authentication".to_owned(),
            CredentialSource::Static {
                schema: crate::connections::schema::empty(),
            },
            mcp(McpCredential::None),
        ),
        Access::Dynamic => (
            "oauth",
            format!("Sign in with {}", server.name),
            oauth(OAuth {
                client: OAuthClient::Dynamic,
                client_auth: ClientAuth::None,
                ..OAuth::standard("")
            }),
            mcp(McpCredential::Bearer),
        ),
        Access::App {
            authorize,
            token,
            scopes,
            client_auth,
        } => (
            "oauth_app",
            "OAuth application".to_owned(),
            oauth(OAuth {
                authorization_url: Some((*authorize).into()),
                scopes: scopes.iter().map(|s| (*s).into()).collect(),
                client_auth: *client_auth,
                ..OAuth::standard(token)
            }),
            mcp(McpCredential::Bearer),
        ),
        Access::Header(name) => (
            "api_key",
            "API key".to_owned(),
            api_key("API key"),
            mcp(McpCredential::Header {
                name: (*name).into(),
                prefix: String::new(),
            }),
        ),
        Access::Query(name) => (
            "api_key",
            "API key".to_owned(),
            api_key("API key"),
            mcp(McpCredential::Query {
                name: (*name).into(),
            }),
        ),
        Access::Bearer => (
            "api_key",
            "API key".to_owned(),
            api_key("API key"),
            mcp(McpCredential::Bearer),
        ),
    };
    ConnectionType {
        id: format!("{prefix}{id}"),
        name,
        capabilities: vec![Capability::Tool],
        credential_source,
        mcp,
    }
}

/// Add every catalogue server to the built-in providers: as its own provider, or as further
/// connection types (`mcp_…`) of the built-in provider with the same ID.
pub fn extend(providers: &mut Vec<Provider>) {
    for server in SERVERS {
        match providers.iter_mut().find(|p| p.id == server.id) {
            Some(provider) => {
                let mut typ = connection_type(server, "mcp_");
                typ.name = format!("{} (MCP server)", typ.name);
                provider.connection_types.push(typ);
            }
            None => providers.push(Provider {
                account_name_label: None,
                icon_url: Some(format!("/provider-icons/{}", server.icon)),
                instructions: Some(server.description.into()),
                id: server.id.into(),
                name: server.name.into(),
                kind: ProviderKind::BuiltIn,
                categories: server.categories.iter().map(|c| (*c).into()).collect(),
                connection_types: vec![connection_type(server, "")],
            }),
        }
    }
}

/// The tools a catalogue server advertised when trytilde/api captured it, for showing what a
/// provider offers before anyone connects. Connections use the server's live list instead.
pub fn advertised(id: &str) -> Vec<crate::proto::tilde::types::v1::ToolDefinition> {
    static SNAPSHOT: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("mcp_catalog_tools.json")).unwrap_or_default()
    });
    SNAPSHOT[id]
        .as_array()
        .into_iter()
        .flatten()
        .map(|tool| crate::proto::tilde::types::v1::ToolDefinition {
            name: tool["name"].as_str().unwrap_or_default().into(),
            description: tool["description"].as_str().unwrap_or_default().into(),
            provider_id: id.into(),
            ..Default::default()
        })
        .collect()
}
