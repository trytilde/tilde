//! Git skill sources are read through GitHub's REST API: resolve the ref to a commit and list
//! the commit's tree (two API calls), then download the files of the skills found from raw
//! content, which does not count against the API rate limit. Everything is pinned to the
//! commit, so a sync never mixes files from two pushes.
use super::package::TreeEntry;
use crate::error::Error;
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;

#[derive(Clone)]
pub struct GitHub {
    api: url::Url,
    raw: url::Url,
    token: Option<SecretString>,
    http: reqwest::Client,
}
/// `https://github.com/{owner}/{repo}`, with or without `.git` or a trailing slash.
pub fn repository(url: &str) -> Result<(String, String), Error> {
    let invalid =
        || Error::Invalid("Use a GitHub repository URL: https://github.com/owner/repo".into());
    let parsed = url::Url::parse(url.trim()).map_err(|_| invalid())?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("github.com") {
        return Err(invalid());
    }
    let mut segments = parsed
        .path_segments()
        .ok_or_else(invalid)?
        .filter(|s| !s.is_empty());
    let owner = segments.next().ok_or_else(invalid)?.to_owned();
    let repo = segments
        .next()
        .ok_or_else(invalid)?
        .trim_end_matches(".git")
        .to_owned();
    let safe = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    if segments.next().is_some() || !safe(&owner) || !safe(&repo) {
        return Err(invalid());
    }
    Ok((owner, repo))
}
pub fn valid_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && !value.starts_with('/')
        && !value.contains("..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/'))
}
impl GitHub {
    pub fn new(api: &str, raw: &str, token: Option<SecretString>) -> Result<Self, Error> {
        let api =
            url::Url::parse(api).map_err(|_| Error::Invalid("Invalid GitHub API URL".into()))?;
        let raw = url::Url::parse(raw)
            .map_err(|_| Error::Invalid("Invalid GitHub raw content URL".into()))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("tilde-skills")
            .build()
            .map_err(|_| Error::Invalid("Unable to build the GitHub HTTP client".into()))?;
        Ok(Self {
            api,
            raw,
            token,
            http,
        })
    }
    async fn get(&self, path: &str, accept: &str) -> Result<reqwest::Response, Error> {
        self.fetch(&self.api, path, accept).await
    }
    async fn fetch(
        &self,
        base: &url::Url,
        path: &str,
        accept: &str,
    ) -> Result<reqwest::Response, Error> {
        let mut url = base.clone();
        // The path is set on its own so a query string is not escaped into it.
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        url.set_path(&format!("{}{path}", base.path().trim_end_matches('/')));
        url.set_query((!query.is_empty()).then_some(query));
        let mut request = self
            .http
            .get(url)
            .header("accept", accept)
            .header("x-github-api-version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.bearer_auth(token.expose_secret());
        }
        let response = request
            .send()
            .await
            .map_err(|_| Error::Invalid("GitHub is unreachable".into()))?;
        match response.status().as_u16() {
            200 => Ok(response),
            404 => Err(Error::Invalid(
                "GitHub has no such repository, ref or path (private repositories need ENGINE_GITHUB_TOKEN)".into(),
            )),
            403 | 429 => Err(Error::Invalid(
                "GitHub rate limited the sync; configure ENGINE_GITHUB_TOKEN".into(),
            )),
            status => Err(Error::Invalid(format!("GitHub answered {status}"))),
        }
    }
    pub async fn commit(&self, owner: &str, repo: &str, git_ref: &str) -> Result<String, Error> {
        #[derive(serde::Deserialize)]
        struct Commit {
            sha: String,
        }
        let commit: Commit = self
            .get(
                &format!("/repos/{owner}/{repo}/commits/{git_ref}"),
                "application/vnd.github+json",
            )
            .await?
            .json()
            .await
            .map_err(|_| Error::Invalid("GitHub returned an unreadable commit".into()))?;
        if commit.sha.len() != 40 || !commit.sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Invalid("GitHub returned an invalid commit".into()));
        }
        Ok(commit.sha)
    }
    pub async fn tree(
        &self,
        owner: &str,
        repo: &str,
        commit: &str,
    ) -> Result<Vec<TreeEntry>, Error> {
        #[derive(serde::Deserialize)]
        struct Tree {
            tree: Vec<Entry>,
            #[serde(default)]
            truncated: bool,
        }
        #[derive(serde::Deserialize)]
        struct Entry {
            path: String,
            mode: String,
            #[serde(rename = "type")]
            kind: String,
            sha: String,
            #[serde(default)]
            size: u64,
        }
        let tree: Tree = self
            .get(
                &format!("/repos/{owner}/{repo}/git/trees/{commit}?recursive=1"),
                "application/vnd.github+json",
            )
            .await?
            .json()
            .await
            .map_err(|_| Error::Invalid("GitHub returned an unreadable tree".into()))?;
        if tree.truncated {
            return Err(Error::Invalid(
                "The repository is too large to list; narrow it with a path".into(),
            ));
        }
        Ok(tree
            .tree
            .into_iter()
            .filter(|e| e.kind == "blob")
            .map(|e| TreeEntry {
                executable: e.mode == "100755",
                symlink: e.mode == "120000",
                path: e.path,
                size: e.size,
                id: e.sha,
            })
            .collect())
    }
    /// One file at the commit, by its repository path.
    pub async fn file(
        &self,
        owner: &str,
        repo: &str,
        commit: &str,
        path: &str,
        limit: u64,
    ) -> Result<Vec<u8>, Error> {
        let encoded: Vec<String> = path
            .split('/')
            .map(|s| {
                url::form_urlencoded::byte_serialize(s.as_bytes())
                    .collect::<String>()
                    .replace('+', "%20")
            })
            .collect();
        let mut response = self
            .fetch(
                &self.raw,
                &format!("/{owner}/{repo}/{commit}/{}", encoded.join("/")),
                "application/octet-stream",
            )
            .await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Error::Invalid("GitHub closed a file download".into()))?
        {
            if bytes.len() as u64 + chunk.len() as u64 > limit {
                return Err(Error::Invalid("A skill file is larger than allowed".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}
