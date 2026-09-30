//! The skill catalog: groups built into the engine and managed providers.
//!
//! A built-in group's skills are directories under `catalog/` embedded at compile time; adding
//! one means adding its files here and listing it in its group. Startup re-syncs every enabled
//! built-in group, so a new engine version brings new versions of the skills it changed.
//!
//! A managed provider is a trusted GitHub repository at a branch, narrowed by path scopes. It
//! syncs through the GitHub client on enable, hourly and on request, never at startup. Before
//! it is enabled, its catalog panel previews the skills live (cached, nothing stored).
pub struct Group {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Sections the catalog page.
    pub category: &'static str,
    /// Served by the web app from `web/public`.
    pub icon_url: &'static str,
    pub origin: Origin,
}
pub enum Origin {
    Builtin(&'static [&'static [(&'static str, &'static str)]]),
    Repository(Repository),
}
pub struct Repository {
    pub url: &'static str,
    pub branch: &'static str,
    /// Repository paths allowed to contribute skills; empty allows the whole tree. Entries
    /// ending in `/` are directory prefixes, others exact file paths. Exact files (such as
    /// `.mcp.json`) never hold a SKILL.md, so they contribute no skills.
    pub include: &'static [&'static str],
    /// Directory prefixes outside the provider's trust boundary.
    pub exclude: &'static [&'static str],
}
impl Repository {
    /// Whether a repository file is within the provider's scopes. Translated copies under
    /// `translations/` are always left out, as they repeat the originals' skill names.
    pub fn allows(&self, path: &str) -> bool {
        let included = self.include.is_empty()
            || self.include.iter().any(|scope| {
                if scope.ends_with('/') {
                    path.starts_with(scope)
                } else {
                    path == *scope
                }
            });
        included
            && !path.starts_with("translations/")
            && !self.exclude.iter().any(|prefix| path.starts_with(prefix))
    }
}
macro_rules! skill {
    ($id:literal) => {
        &[(
            concat!($id, "/SKILL.md"),
            include_str!(concat!("catalog/", $id, "/SKILL.md")),
        )]
    };
}
const TILDE_ICON: &str = "/tilde-mark.svg";
pub const GROUPS: &[Group] = &[
    Group {
        id: "tilde-runtime",
        name: "Tilde runtime",
        description: "How to use Tilde's channels, goals and tasks, inference and attachments.",
        category: "Tilde",
        icon_url: TILDE_ICON,
        origin: Origin::Builtin(&[
            skill!("tilde-channels"),
            skill!("tilde-goals-and-tasks"),
            skill!("tilde-inference"),
            skill!("tilde-attachments"),
        ]),
    },
    Group {
        id: "tilde-working-style",
        name: "Tilde working style",
        description: "Reply and hand-off habits for chat agents.",
        category: "Tilde",
        icon_url: TILDE_ICON,
        origin: Origin::Builtin(&[skill!("concise-chat-replies"), skill!("escalation-handoff")]),
    },
    // The Tilde-owned skills the Tilde API serves; each defers to its hosted document.
    Group {
        id: "tilde-platform",
        name: "Tilde platform",
        description: "Building and operating agents on the Tilde platform.",
        category: "Tilde",
        icon_url: TILDE_ICON,
        origin: Origin::Builtin(&[
            skill!("enable-connections"),
            skill!("tool-provider-integration"),
            skill!("chatkit-signals-messaging"),
            skill!("create-deploy-agent"),
            skill!("slackbot-agent"),
            skill!("github-agent"),
            skill!("agent-to-agent-communication"),
            skill!("ai-sdk-compatible-agent"),
        ]),
    },
    // Managed providers: trusted repositories whose skills sync once enabled.
    Group {
        id: "anthropic",
        name: "Anthropic",
        description: "Reference agent skill authoring patterns and progressive-disclosure examples.",
        category: "AI",
        icon_url: "/skill-provider-icons/anthropic.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/anthropics/anthropic-cookbook",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "microsoft",
        name: "Microsoft",
        description: "Skills for configuring enterprise SaaS and Microsoft-oriented MCP workflows.",
        category: "AI",
        icon_url: "/skill-provider-icons/microsoft.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/microsoft/ai-agents-for-beginners",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "cloudflare",
        name: "Cloudflare",
        description: "Operational skills for custom tool serving and edge-adjacent MCP workflows.",
        category: "Cloud and infrastructure",
        icon_url: "/skill-provider-icons/cloudflare.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/cloudflare/agents",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "aws",
        name: "AWS",
        description: "Official AWS agent skills for building, deploying, operating, and securing workloads with the Agent Toolkit for AWS.",
        category: "Cloud and infrastructure",
        icon_url: "/skill-provider-icons/aws.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/aws/agent-toolkit-for-aws",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "cursor",
        name: "Cursor",
        description: "Cursor-built skills, including documentation and pull-request review canvases.",
        category: "Developer tools",
        icon_url: "/skill-provider-icons/cursor.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/cursor/plugins",
            branch: "main",
            include: &[],
            exclude: &["third_party/"],
        }),
    },
    Group {
        id: "notion",
        name: "Notion",
        description: "Official Notion skills for workspace search, knowledge work, and MCP-assisted workflows.",
        category: "Productivity",
        icon_url: "/skill-provider-icons/notion.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/makenotion/cursor-notion-plugin",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "granola",
        name: "Granola",
        description: "Official Granola skills for meeting notes, transcripts, and follow-up workflows.",
        category: "Productivity",
        icon_url: "/skill-provider-icons/granola.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/granola-inc/granola-cursor-plugin",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "parallel",
        name: "Parallel",
        description: "Official Parallel skills for web research and task-oriented agent workflows.",
        category: "Research",
        icon_url: "/skill-provider-icons/parallel.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/parallel-web/parallel-cursor-plugin",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "superpowers",
        name: "Superpowers",
        description: "Agent development workflows for planning, debugging, testing, review, and delivery.",
        category: "Developer tools",
        icon_url: "/skill-provider-icons/superpowers.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/obra/superpowers",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "browserbase",
        name: "Browserbase",
        description: "Browser automation and cloud-browser workflow skills maintained by Browserbase.",
        category: "Developer tools",
        icon_url: "/skill-provider-icons/browserbase.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/browserbase/browse-plugin",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "cua",
        name: "Cua",
        description: "Official Cua guidance for portable GUI automation workflows.",
        category: "Developer tools",
        icon_url: "/skill-provider-icons/cua.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/trycua/cua",
            branch: "main",
            include: &["skills/gui-automation/"],
            exclude: &[],
        }),
    },
    Group {
        id: "apollo",
        name: "Apollo.io",
        description: "Official Apollo skills for sales research, enrichment, outreach, and pipeline workflows.",
        category: "Sales",
        icon_url: "/skill-provider-icons/apollo.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/apolloio/apollo-mcp-plugin",
            branch: "main",
            include: &[],
            exclude: &[],
        }),
    },
    Group {
        id: "zoom",
        name: "Zoom",
        description: "Official Zoom skills for hosted MCP servers, REST APIs, SDKs, OAuth, webhooks, and collaboration workflows.",
        category: "Productivity",
        icon_url: "/skill-provider-icons/zoom.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/zoom/skills",
            branch: "main",
            include: &["skills/", ".mcp.json"],
            exclude: &[],
        }),
    },
    Group {
        id: "stripe",
        name: "Stripe",
        description: "Official Stripe skills for payments, billing, Connect, Apps, documentation, and integration upgrades.",
        category: "Payments",
        icon_url: "/skill-provider-icons/stripe.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/stripe/agent-toolkit",
            branch: "main",
            include: &["providers/codex/plugin/skills/"],
            exclude: &[],
        }),
    },
    Group {
        id: "neon",
        name: "Neon",
        description: "Official Neon skills for claimable Postgres workflows and database logging practices.",
        category: "Cloud and infrastructure",
        icon_url: "/skill-provider-icons/neon.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/neondatabase-labs/mcp-server-neon",
            branch: "main",
            include: &[".agents/skills/"],
            exclude: &[],
        }),
    },
    Group {
        id: "vercel",
        name: "Vercel",
        description: "Official Vercel skills for deployment, CLI workflows, framework practices, and optimization.",
        category: "Cloud and infrastructure",
        icon_url: "/skill-provider-icons/vercel.svg",
        origin: Origin::Repository(Repository {
            url: "https://github.com/vercel-labs/agent-skills",
            branch: "main",
            include: &["skills/"],
            exclude: &[],
        }),
    },
    Group {
        id: "qm",
        name: "YC Software QM",
        description: "Portable design-system and visual-quality skills published in QM's public skill seed catalogue.",
        category: "Design",
        icon_url: "/skill-provider-icons/qm.png",
        origin: Origin::Repository(Repository {
            url: "https://github.com/yc-software/qm",
            branch: "main",
            include: &[
                "skills-seed/popular-web-designs/",
                "skills-seed/taste-skill/",
            ],
            exclude: &[],
        }),
    },
];
pub fn get(id: &str) -> Option<&'static Group> {
    GROUPS.iter().find(|g| g.id == id)
}
