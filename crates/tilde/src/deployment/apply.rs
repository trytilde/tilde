//! Declarative apply of an agent's deployment (see `crate::apply`): the customer deployed the
//! code already; apply registers it once per external ID, in the same transaction as the config.
use super::{Deployments, Prepared, RegisterDeployment, Registered};
use crate::apply::Change;
use crate::database::Transaction;
use crate::error::Error;
use crate::proto::tilde::{management::v1 as m, types::v1 as types};
use secrecy::SecretString;
use uuid::Uuid;

/// The registration a spec asks for, with the request's commit details.
pub(crate) fn registration(
    spec: &m::DeploymentSpec,
    request: &m::ApplyRequest,
) -> RegisterDeployment {
    RegisterDeployment {
        source: types::DeploymentSource::Ci,
        target: spec.target.as_known().unwrap_or_default(),
        target_reference: spec.target_reference.clone(),
        repository: request.repository.clone(),
        commit_sha: request.commit_sha.clone(),
        external_id: Some(spec.external_id.clone()).filter(|id| !id.trim().is_empty()),
        label: spec.label.clone(),
        commit_message: request.commit_message.clone(),
        branch: request.branch.clone(),
        commit_author: request.commit_author.clone(),
        declarations: spec.declarations.as_option().cloned().unwrap_or_default(),
    }
}

impl Deployments {
    pub(crate) async fn apply(
        &self,
        tx: &Transaction<'_>,
        agent: Uuid,
        agent_key: &str,
        prepared: Prepared,
    ) -> Result<(Change, Option<(Uuid, SecretString)>), Error> {
        let key = format!(
            "{agent_key}/{}",
            prepared.external.as_deref().unwrap_or_default()
        );
        match self.register_in(tx, agent, &prepared).await? {
            Registered::Existing(_) => Ok((Change::existing("deployment", key), None)),
            Registered::Created(id, token) => {
                let mut change = Change::create("deployment", key);
                change.field("target", "", prepared.target);
                if let Some(reference) = &prepared.reference {
                    change.field("target_reference", "", reference);
                    // arn:aws:lambda:region:account:function:name[:qualifier]
                    if reference.split(':').count() < 8 {
                        change.note = "Unqualified Lambda ARN: the deployment follows $LATEST; \
                            register a published version for a fixed deployment"
                            .into();
                    }
                }
                Ok((change, Some((id, token))))
            }
        }
    }
}
