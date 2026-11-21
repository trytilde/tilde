//! Secrets: named values people set in the app and specs reference by name, such as a sandbox
//! blueprint's environment variable. A value is sealed by the encryption crate when it is set,
//! opened only for the sandbox process the variable is wired into, and never returned through
//! management. A secret a blueprint references cannot be deleted.
pub mod db;
pub mod rpc;

use crate::database::Pool;
use crate::encryption::{Encryption, SecretBinding};
use crate::error::Error;
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use uuid::Uuid;

pub struct Secret {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Blueprints whose environment references it, by ID and name.
    pub blueprints: Vec<(Uuid, String)>,
}

/// The slot a secret's value is sealed in.
pub fn binding(id: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "secret",
        resource_id: id,
        name: "value",
    }
}
/// Environment-variable style, so a secret name is also a valid variable name.
pub fn valid_name(name: &str) -> bool {
    name.len() <= 128
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Clone)]
pub struct Secrets {
    pool: Pool,
    crypto: Arc<Encryption>,
}
impl Secrets {
    pub fn new(pool: Pool, crypto: Arc<Encryption>) -> Self {
        Self { pool, crypto }
    }
    pub async fn list(&self) -> Result<Vec<Secret>, Error> {
        let rows = db::secret_list_all(&self.pool.get().await?).await?;
        Ok(rows
            .into_iter()
            .map(|r| Secret {
                id: r.id,
                name: r.name,
                description: r.description,
                created_at: r.created_at,
                updated_at: r.updated_at,
                blueprints: r.blueprint_ids.into_iter().zip(r.blueprints).collect(),
            })
            .collect())
    }
    pub async fn get(&self, id: Uuid) -> Result<db::SecretRow, Error> {
        db::secret_get_opt(&self.pool.get().await?, id)
            .await?
            .ok_or(Error::Missing("Secret"))
    }
    /// Adds the secret `name`, or replaces its description and, when given, its value; with
    /// `id`, replaces that secret. The plaintext is dropped (and zeroized) once sealed.
    pub async fn set(
        &self,
        id: Option<Uuid>,
        name: &str,
        value: Option<SecretString>,
        description: &str,
    ) -> Result<Secret, Error> {
        let name = name.trim();
        if !valid_name(name) {
            return Err(Error::Invalid(
                "A secret name is letters, digits and underscores, not starting with a digit (at most 128)".into(),
            ));
        }
        let description = description.trim();
        if description.chars().count() > 500 {
            return Err(Error::Invalid(
                "A secret description is at most 500 characters".into(),
            ));
        }
        if value
            .as_ref()
            .is_some_and(|v| v.expose_secret().len() > 32 * 1024)
        {
            return Err(Error::Invalid("A secret's value is at most 32 KiB".into()));
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let found = db::secret_by_name_opt(&tx, name.trim()).await?;
        let id = match id.or(found) {
            Some(id) => {
                let sealed = value
                    .map(|value| self.crypto.seal(binding(id), &value))
                    .transpose()?
                    .map(|sealed| sealed.into_bytes());
                db::secret_update_execute(&tx, id, sealed.as_deref(), description).await?;
                id
            }
            None => {
                let value =
                    value.ok_or_else(|| Error::Invalid("A new secret needs a value".into()))?;
                let id = Uuid::new_v4();
                let sealed = self.crypto.seal(binding(id), &value)?.into_bytes();
                drop(value);
                db::secret_insert_execute(&tx, id, name, description, &sealed)
                    .await
                    .map_err(|e| match e.is_unique_violation() {
                        true => Error::Exists("A secret with this name exists".into()),
                        false => e.into(),
                    })?;
                id
            }
        };
        tx.commit().await?;
        drop(client);
        self.list()
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(Error::Missing("Secret"))
    }
    pub async fn delete(&self, id: Uuid) -> Result<(), Error> {
        match db::secret_delete_execute(&self.pool.get().await?, id).await {
            Err(e) if e.is_foreign_key_violation() => Err(Error::Exists(
                "Sandbox blueprints or environments reference this secret; remove those references first"
                    .into(),
            )),
            Err(e) => Err(e.into()),
            Ok(0) => Err(Error::Missing("Secret")),
            Ok(_) => Ok(()),
        }
    }
}
