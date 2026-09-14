#[allow(dead_code)]
mod common;

use serde_json::json;
use std::{borrow::Cow, collections::BTreeMap};
use tilde::iam::capabilities::{Capabilities, Capability, Reach};
use tilde::proto::tilde::types::v1 as types;
use uuid::Uuid;

fn grant(action: Capability, reach: Reach) -> Capabilities {
    Capabilities(BTreeMap::from([(action, reach)]))
}

#[test]
fn rpc_modes_enforce_the_permission_kind_and_target_rules() {
    for action in [
        Capability::AgentsRead,
        Capability::AgentsCreate,
        Capability::AgentsUpdate,
        Capability::AgentsDelete,
        Capability::AgentsInvoke,
        Capability::AgentsGrant,
        Capability::ThreadRead,
        Capability::WorkRead,
        Capability::WorkWrite,
        Capability::RunUpdate,
        Capability::ToolsInvoke,
    ] {
        for scope in [
            Reach::No,
            Reach::Yes,
            Reach::None,
            Reach::All,
            Reach::Selected {
                ids: vec![Uuid::new_v4().to_string()],
            },
        ] {
            let valid = match scope {
                Reach::No | Reach::Yes => !action.is_targeted(),
                _ => action.is_targeted(),
            };
            let caps = grant(action, scope);
            let decoded = Capabilities::from_wire(caps.wire());
            assert_eq!(decoded.is_ok(), valid, "{caps:?}");
            if valid {
                let stored = serde_json::to_value(decoded.unwrap()).unwrap();
                assert_eq!(
                    serde_json::from_value::<Capabilities>(stored).unwrap(),
                    caps
                );
            } else {
                assert!(!caps.grants(action));
                assert!(!caps.permits(action, ""));
            }
        }
    }
    for (mode, ids) in [
        (
            types::TargetSelection::All.into(),
            vec![Uuid::new_v4().to_string()],
        ),
        (
            types::TargetSelection::Selected.into(),
            vec!["not-a-uuid".into()],
        ),
        (types::TargetSelection::Selected.into(), vec![String::new()]),
        (types::TargetSelection::Unspecified.into(), vec![]),
        (buffa::EnumValue::Unknown(99), vec![]),
    ] {
        let wire = types::Capabilities {
            agents_read: types::TargetPermission {
                mode,
                ids,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        assert!(Capabilities::from_wire(wire).is_err());
    }
    assert!(
        Capabilities::from_wire(types::Capabilities {
            agents_create: buffa::EnumValue::Unknown(99),
            ..Default::default()
        })
        .is_err()
    );
    assert!(serde_json::from_value::<types::Capabilities>(json!({"agentsCreate":"yes"})).is_err());
}

#[test]
fn renewal_intersections_cannot_widen_binary_or_targeted_permissions() {
    let a = Uuid::new_v4().to_string();
    let b = Uuid::new_v4().to_string();
    for (action, scopes) in [
        (Capability::AgentsCreate, vec![Reach::No, Reach::Yes]),
        (
            Capability::AgentsRead,
            vec![
                Reach::None,
                Reach::All,
                Reach::Selected { ids: vec![] },
                Reach::Selected {
                    ids: vec![a.clone()],
                },
                Reach::Selected {
                    ids: vec![b.clone()],
                },
                Reach::Selected {
                    ids: vec![a.clone(), b.clone()],
                },
            ],
        ),
    ] {
        let mut grants = vec![Capabilities::default()];
        grants.extend(scopes.into_iter().map(|scope| grant(action, scope)));
        for original in &grants {
            for ceiling in &grants {
                let renewed = original.intersect(ceiling);
                renewed.validate().unwrap();
                assert!(renewed.subset_of(original));
                assert!(renewed.subset_of(ceiling));
                for id in [&a, &b] {
                    assert_eq!(
                        renewed.permits(action, id),
                        original.permits(action, id) && ceiling.permits(action, id)
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn migration_preserves_existing_denials_grants_and_selected_targets() {
    let db = common::Database::unmigrated().await;
    let all = sqlx::migrate!("../../migrations");
    let before = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|migration| migration.version < 20260910060000)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    before.run(&db.pool).await.unwrap();
    let id = Uuid::new_v4();
    let target = Uuid::new_v4().to_string();
    let old = json!({
        "agents.create": {"mode": "any"}, "thread.read": {"mode": "none"},
        "agents.read": {"mode": "any"}, "agents.delete": {"mode": "none"},
        "agents.invoke": {"mode": "only", "ids": [target]},
        "tools.invoke": {"mode": "only", "ids": ["react_to_message"]}
    });
    sqlx::query("INSERT INTO agents (id, name, webhook_signing_key, capabilities) VALUES ($1, 'Migration fixture', $2, $3)")
        .bind(id).bind(vec![0u8; 46]).bind(old).execute(&db.pool).await.unwrap();
    all.run(&db.pool).await.unwrap();
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT capabilities FROM agents WHERE id = $1")
            .bind(id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let caps: Capabilities = serde_json::from_value(stored).unwrap();
    caps.validate().unwrap();
    assert!(caps.grants(Capability::AgentsCreate));
    assert!(!caps.grants(Capability::ThreadRead));
    assert!(caps.permits(Capability::AgentsRead, &target));
    assert!(!caps.permits(Capability::AgentsDelete, &target));
    assert!(caps.permits(Capability::AgentsInvoke, &target));
    assert!(!caps.permits(Capability::AgentsInvoke, &Uuid::new_v4().to_string()));
    assert!(caps.permits(Capability::ToolsInvoke, "react_to_message"));
    assert!(!caps.permits(Capability::ToolsInvoke, "send_message"));
    assert!(!caps.grants(Capability::WorkWrite));
    assert_eq!(Capabilities::from_wire(caps.wire()).unwrap(), caps);
    db.close().await;
}
