//! A connection in use is never disconnected: an agent's assignment and a routine on its signals
//! keep it, the refusal names both, and once each is removed (or their agent deleted) it
//! disconnects. A routine created while a disconnect races it never lands on a disconnected
//! connection. Real Postgres; the connections, routines and agents services together.
mod common;
use common::invocation::Fixture;
use tilde::connections::model::{Assignment, Capability};
use tilde::routines::{RoutineInput, Routines, Trigger};
use uuid::Uuid;

/// One of a GitHub connection's signals.
const SIGNAL: &str = "github.issue.opened";

async fn github(fx: &Fixture) -> Uuid {
    let id = Uuid::new_v4();
    fx.within(async {
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                "INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,'acme','github','pat','ready')",
                &[&id],
            )
            .await
            .unwrap();
    })
    .await;
    id
}

async fn set_ready(fx: &Fixture, connection: Uuid) {
    fx.within(async {
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE connections SET status='ready' WHERE id=$1",
                &[&connection],
            )
            .await
            .unwrap();
    })
    .await;
}

fn on_signal(connection: Uuid, signal_type: &str, name: &str) -> RoutineInput {
    RoutineInput {
        name: name.into(),
        prompt: "Triage it.".into(),
        thread_title: String::new(),
        enabled: true,
        trigger: Trigger::Signal {
            connection_id: connection,
            signal_type: signal_type.into(),
            filters: vec![],
        },
    }
}

fn precondition(error: tilde::error::Error) -> String {
    let error = connectrpc::ConnectError::from(error);
    assert_eq!(
        error.code,
        connectrpc::ErrorCode::FailedPrecondition,
        "{error:?}"
    );
    error.message.unwrap_or_default()
}

#[tokio::test]
async fn a_connection_disconnects_only_once_nothing_uses_it() {
    let fx = Fixture::new(61, Default::default(), Default::default()).await;
    let connection = github(&fx).await;
    let routines = Routines::new(fx.db.pool.clone(), fx.chat.clone());
    let skills = Assignment {
        capability: Capability::Skills,
        agent_id: fx.agent,
        alias: None,
    };
    fx.within(fx.connections.assign(connection, &skills))
        .await
        .unwrap();
    let signal = SIGNAL.to_string();
    let routine = fx
        .within(routines.create(fx.agent, on_signal(connection, &signal, "On issues")))
        .await
        .unwrap();

    let usages = fx
        .within(fx.connections.usages(&[connection]))
        .await
        .unwrap();
    let kinds: Vec<(&str, &str)> = usages
        .iter()
        .map(|u| (u.kind.as_str(), u.detail.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("agent", "skills"),
            ("routine", &*format!("{}, {signal}", usages[0].name))
        ]
    );
    let refused = precondition(
        fx.within(fx.connections.disconnect(connection))
            .await
            .unwrap_err(),
    );
    assert!(
        refused.contains(&format!("agent {} (skills)", usages[0].name)),
        "{refused}"
    );
    assert!(refused.contains("routine On issues"), "{refused}");

    // One at a time; the routine's still holds it.
    fx.within(fx.connections.unassign(connection, &skills))
        .await
        .unwrap();
    let refused = precondition(
        fx.within(fx.connections.disconnect(connection))
            .await
            .unwrap_err(),
    );
    assert!(!refused.contains("(skills)"), "{refused}");
    fx.within(routines.delete(routine.id)).await.unwrap();
    fx.within(fx.connections.disconnect(connection))
        .await
        .unwrap();

    // Disconnected, nothing puts it to use until it is reconnected.
    precondition(
        fx.within(fx.connections.assign(connection, &skills))
            .await
            .unwrap_err(),
    );
    precondition(
        fx.within(routines.create(fx.agent, on_signal(connection, &signal, "Again")))
            .await
            .err()
            .expect("refused"),
    );

    // A deleted agent's routines go with it, and so does their hold on the connection.
    set_ready(&fx, connection).await;
    fx.within(routines.create(fx.agent, on_signal(connection, &signal, "Later")))
        .await
        .unwrap();
    fx.within(fx.agents.pause(fx.agent)).await.unwrap();
    fx.within(fx.agents.delete(fx.agent)).await.unwrap();
    fx.within(fx.connections.disconnect(connection))
        .await
        .unwrap();
}

/// Routines created while the connection is disconnected: each pair ends with the routine and
/// a ready connection, or the connection disconnected and no routine; never both.
#[tokio::test]
async fn a_routine_racing_a_disconnect_never_lands_on_a_disconnected_connection() {
    let fx = Fixture::new(62, Default::default(), Default::default()).await;
    let connection = github(&fx).await;
    let routines = Routines::new(fx.db.pool.clone(), fx.chat.clone());
    let signal = SIGNAL.to_string();
    for n in 0..10 {
        set_ready(&fx, connection).await;
        let name = format!("Race {n}");
        let (created, disconnected) = tokio::join!(
            fx.within(routines.create(fx.agent, on_signal(connection, &signal, &name))),
            fx.within(fx.connections.disconnect(connection)),
        );
        match (created, disconnected) {
            (Ok(routine), Err(refused)) => {
                assert!(precondition(refused).contains(&name));
                fx.within(routines.delete(routine.id)).await.unwrap();
            }
            (Err(refused), Ok(())) => {
                precondition(refused);
            }
            (created, disconnected) => {
                panic!(
                    "not serialised: {:?} {:?}",
                    created.map(|r| r.id),
                    disconnected
                )
            }
        }
    }
}
