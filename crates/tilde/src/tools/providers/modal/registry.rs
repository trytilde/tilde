//! A blueprint image's tag resolved to its digest before Modal builds from it. Modal caches an
//! image on its Dockerfile commands, so `FROM repo:latest` keeps whatever it built first; with
//! the digest in the `FROM` line, a moved tag is a new image. Public registries only, as Modal
//! pulls them (anonymously, with the registry's token handshake), through a client that reaches
//! public addresses only: the reference is the blueprint author's.

use crate::chat::providers::files::public_client;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);
const MANIFESTS: &str = "application/vnd.oci.image.index.v1+json, \
    application/vnd.docker.distribution.manifest.list.v2+json, \
    application/vnd.oci.image.manifest.v1+json, \
    application/vnd.docker.distribution.manifest.v2+json";

/// `image` pinned to its digest (`name@sha256:…`), or `image` as it is when it is pinned already
/// or the registry cannot say (logged: Modal then builds from the tag).
pub async fn pinned(image: &str) -> String {
    let Some(reference) = Reference::parse(image) else {
        return image.to_owned();
    };
    match digest(&reference).await {
        Ok(digest) => format!("{}@{digest}", reference.name),
        Err(error) => {
            tracing::warn!(%image, %error, "the image's digest is unknown; Modal builds from its tag");
            image.to_owned()
        }
    }
}

/// A registry reference with a tag (none means `latest`).
#[derive(Debug, PartialEq)]
struct Reference {
    /// The registry's API host (`registry-1.docker.io` for Docker Hub).
    host: String,
    /// The repository in the registry's API (`library/python` for `python`).
    repository: String,
    tag: String,
    /// The reference as written, without its tag.
    name: String,
}

impl Reference {
    /// None for a reference pinned by digest already, or one that is not a registry reference.
    fn parse(image: &str) -> Option<Self> {
        if image.contains('@') || image.is_empty() || image.contains(char::is_whitespace) {
            return None;
        }
        // A tag is a `:` after the last `/` (a `:` before it is a registry port).
        let slash = image.rfind('/').map_or(0, |i| i + 1);
        let (name, tag) = match image[slash..].rfind(':') {
            Some(i) => (&image[..slash + i], &image[slash + i + 1..]),
            None => (image, "latest"),
        };
        if name.is_empty() || tag.is_empty() {
            return None;
        }
        // The first component is a registry when it looks like a host.
        let (host, repository) = match name.split_once('/') {
            Some((first, rest)) if first.contains(['.', ':']) || first == "localhost" => {
                (first.to_owned(), rest.to_owned())
            }
            _ if name.contains('/') => ("registry-1.docker.io".to_owned(), name.to_owned()),
            _ => ("registry-1.docker.io".to_owned(), format!("library/{name}")),
        };
        let host = if host == "docker.io" {
            "registry-1.docker.io".to_owned()
        } else {
            host
        };
        Some(Self {
            host,
            repository,
            tag: tag.to_owned(),
            name: name.to_owned(),
        })
    }
}

/// The manifest's digest, from the registry's `Docker-Content-Digest` header.
async fn digest(r: &Reference) -> Result<String, String> {
    let url = url::Url::parse(&format!(
        "https://{}/v2/{}/manifests/{}",
        r.host, r.repository, r.tag
    ))
    .map_err(|e| e.to_string())?;
    let client = public_client(&url, TIMEOUT)
        .await
        .map_err(|e| e.to_string())?;
    let head = |token: Option<String>| {
        let mut request = client.head(url.clone()).header("Accept", MANIFESTS);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send()
    };
    let mut response = head(None).await.map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        let challenge = response
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .ok_or("the registry asked for credentials without a challenge")?;
        let token = token(challenge).await?;
        response = head(Some(token)).await.map_err(|e| e.to_string())?;
    }
    if !response.status().is_success() {
        return Err(format!("the registry answered {}", response.status()));
    }
    let digest = response
        .headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let valid = digest
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()));
    if !valid {
        return Err("the registry sent no sha256 digest".into());
    }
    Ok(digest.to_owned())
}

/// An anonymous pull token for a `Bearer realm="…",service="…",scope="…"` challenge.
async fn token(challenge: &str) -> Result<String, String> {
    let params = challenge
        .strip_prefix("Bearer ")
        .ok_or("the registry's challenge is not Bearer")?;
    let mut realm = None;
    let mut query = Vec::new();
    for part in params.split(',') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        let value = value.trim_matches('"');
        match key {
            "realm" => realm = Some(value),
            "service" | "scope" => query.push((key, value)),
            _ => {}
        }
    }
    let mut url = url::Url::parse(realm.ok_or("the registry's challenge has no realm")?)
        .map_err(|e| e.to_string())?;
    if url.scheme() != "https" {
        return Err("the registry's token realm is not HTTPS".into());
    }
    url.query_pairs_mut().extend_pairs(query);
    let client = public_client(&url, TIMEOUT)
        .await
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    body["token"]
        .as_str()
        .or(body["access_token"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| "the registry sent no token".into())
}

#[cfg(test)]
mod tests {
    use super::Reference;

    fn parsed(image: &str) -> Option<(String, String, String, String)> {
        Reference::parse(image).map(|r| (r.host, r.repository, r.tag, r.name))
    }

    #[test]
    fn references_name_their_registry_repository_and_tag() {
        let r = |host: &str, repository: &str, tag: &str, name: &str| {
            Some((host.into(), repository.into(), tag.into(), name.into()))
        };
        assert_eq!(
            parsed("python"),
            r("registry-1.docker.io", "library/python", "latest", "python")
        );
        assert_eq!(
            parsed("docker.io/acme/tools:1.2"),
            r(
                "registry-1.docker.io",
                "acme/tools",
                "1.2",
                "docker.io/acme/tools"
            )
        );
        assert_eq!(
            parsed("ghcr.io/trytilde/sandbox:latest"),
            r(
                "ghcr.io",
                "trytilde/sandbox",
                "latest",
                "ghcr.io/trytilde/sandbox"
            )
        );
        // A port is not a tag.
        assert_eq!(
            parsed("registry.example.com:5000/team/img"),
            r(
                "registry.example.com:5000",
                "team/img",
                "latest",
                "registry.example.com:5000/team/img"
            )
        );
        assert_eq!(parsed("ghcr.io/trytilde/sandbox@sha256:abc"), None);
        assert_eq!(parsed("img:"), None);
    }
}
