//! What a skill is on disk: a directory holding a `SKILL.md` and whatever files it uses. The
//! rules follow Tilde's hosted skills: safe relative paths, no symlinks, 1 MiB for SKILL.md,
//! 10 MiB per file, 64 MiB and 2048 files per skill. A skill owns every file under its
//! directory except those under a nested skill.
use crate::error::Error;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const ENTRYPOINT: &str = "SKILL.md";
pub const MAX_FILES: usize = 2048;
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_ENTRYPOINT_BYTES: u64 = 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
/// UTF-8 files up to this size are kept inline in Postgres as well as in object storage.
pub const MAX_INLINE_BYTES: usize = 256 * 1024;

pub fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= 64
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}
pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !segment.bytes().any(|b| b.is_ascii_control() || b == b'\\')
        })
}
/// Lowercase, runs of anything else become `-`, at most 64 characters.
pub fn normalize_name(value: &str) -> String {
    let mut name = String::new();
    for c in value.trim().chars() {
        if c.is_ascii_alphanumeric() {
            name.push(c.to_ascii_lowercase());
        } else if !name.ends_with('-') && !name.is_empty() {
            name.push('-');
        }
    }
    name.truncate(64);
    name.trim_end_matches('-').to_owned()
}
/// `name:` and `description:` from the SKILL.md front matter.
pub fn front_matter(skill_md: &str) -> (Option<String>, String) {
    let mut lines = skill_md.lines().peekable();
    if lines.next().map(str::trim) != Some("---") {
        return (None, String::new());
    }
    let (mut name, mut description) = (None, String::new());
    while let Some(line) = lines.next() {
        if line.trim() == "---" {
            break;
        }
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        if !matches!(key, "name" | "description") {
            continue;
        }
        // Indented lines continue the value: a `>`/`|` block scalar or a wrapped plain scalar.
        let mut continued = Vec::new();
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() || next.starts_with([' ', '\t']) {
                continued.push(next.trim());
                lines.next();
            } else {
                break;
            }
        }
        let rest = rest.trim();
        let value = match rest.chars().next() {
            Some('|') => continued.join("\n").trim().to_owned(),
            Some('>') => continued.join(" ").trim().to_owned(),
            _ => std::iter::once(rest)
                .chain(continued)
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_owned(),
        };
        let value = value.trim_matches(['"', '\'']).to_owned();
        if key == "name" {
            name = Some(value).filter(|n| !n.is_empty());
        } else {
            description = value.chars().take(1024).collect();
        }
    }
    (name, description)
}
/// From the extension alone; unknown files are opaque bytes.
pub fn media_type(path: &str) -> &'static str {
    let extension = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match extension.as_deref() {
        Some("md" | "markdown") => "text/markdown; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("py") => "text/x-python; charset=utf-8",
        Some("js" | "mjs" | "cjs") => "text/javascript; charset=utf-8",
        Some("ts" | "tsx") => "text/x-typescript; charset=utf-8",
        Some("sh" | "bash") => "application/x-sh",
        Some("json") => "application/json",
        Some("yaml" | "yml") => "application/yaml",
        Some("toml") => "application/toml",
        Some("csv") => "text/csv; charset=utf-8",
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("xml") => "application/xml",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}
pub fn digest(data: &[u8]) -> Vec<u8> {
    Sha256::digest(data).to_vec()
}
/// Content-addressed, so a file shared by skills or versions is stored once.
pub fn object_key(digest: &[u8]) -> String {
    let hex = hex::encode(digest);
    format!("skills/blobs/sha256/{}/{hex}", &hex[..2])
}

/// One file of a skill, identified by content. `data` is present for files not yet stored.
#[derive(Debug, Clone)]
pub struct File {
    pub path: String,
    pub media_type: String,
    pub size: u64,
    pub digest: Vec<u8>,
    pub executable: bool,
    pub content: Option<String>,
    pub data: Option<Vec<u8>>,
}
impl File {
    pub fn new(path: String, data: Vec<u8>, executable: bool) -> Self {
        let content = (data.len() <= MAX_INLINE_BYTES)
            .then(|| String::from_utf8(data.clone()).ok())
            .flatten();
        Self {
            media_type: media_type(&path).to_owned(),
            size: data.len() as u64,
            digest: digest(&data),
            path,
            executable,
            content,
            data: Some(data),
        }
    }
    /// Stored in object storage: anything not kept inline.
    pub fn needs_object(&self) -> bool {
        self.content.is_none()
    }
    /// The file as UTF-8 text: inline content, or bytes too large to keep inline.
    pub fn text(&self) -> Option<std::borrow::Cow<'_, str>> {
        match (&self.content, &self.data) {
            (Some(text), _) => Some(text.as_str().into()),
            (None, Some(data)) => std::str::from_utf8(data).ok().map(Into::into),
            (None, None) => None,
        }
    }
}
/// A validated skill ready to store: files sorted by path and the hash that names its content.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub description: String,
    pub dir: String,
    pub files: Vec<File>,
    pub hash: Vec<u8>,
}
impl Package {
    pub fn new(name: String, dir: String, mut files: Vec<File>) -> Result<Self, Error> {
        if !valid_name(&name) {
            return Err(Error::Invalid(format!(
                "Skill name {name} must use lowercase letters, digits, '-' or '_' (1-64 characters)"
            )));
        }
        if files.is_empty() || files.len() > MAX_FILES {
            return Err(Error::Invalid(format!(
                "Skill {name} must have 1-{MAX_FILES} files"
            )));
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        if files.windows(2).any(|w| w[0].path == w[1].path) {
            return Err(Error::Invalid(format!("Skill {name} repeats a file path")));
        }
        let mut total = 0;
        let mut hasher = Sha256::new();
        for file in &files {
            if !valid_path(&file.path) {
                return Err(Error::Invalid(format!(
                    "Skill {name} has an unsafe path {}",
                    file.path
                )));
            }
            let cap = if file.path == ENTRYPOINT {
                MAX_ENTRYPOINT_BYTES
            } else {
                MAX_FILE_BYTES
            };
            if file.size > cap {
                return Err(Error::Invalid(format!(
                    "{} in skill {name} is larger than {} MiB",
                    file.path,
                    cap / 1024 / 1024
                )));
            }
            total += file.size;
            hasher.update(file.path.as_bytes());
            hasher.update(b"\0");
            hasher.update(hex::encode(&file.digest).as_bytes());
            hasher.update(if file.executable {
                b"\x001\0"
            } else {
                b"\x000\0"
            });
        }
        if total > MAX_TOTAL_BYTES {
            return Err(Error::Invalid(format!(
                "Skill {name} is larger than 64 MiB"
            )));
        }
        let entry = files
            .iter()
            .find(|f| f.path == ENTRYPOINT)
            .ok_or_else(|| Error::Invalid(format!("Skill {name} needs a SKILL.md")))?;
        let description = entry
            .text()
            .map(|md| front_matter(&md).1)
            .ok_or_else(|| Error::Invalid(format!("SKILL.md of {name} must be UTF-8 text")))?;
        Ok(Self {
            name,
            description,
            dir,
            hash: hasher.finalize().to_vec(),
            files,
        })
    }
}

/// One file of a repository tree, before its bytes are fetched.
#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub path: String,
    pub size: u64,
    pub executable: bool,
    pub symlink: bool,
    /// Git object id, used to fetch and to share identical files across skills.
    pub id: String,
}
/// A skill found in a tree: its directory, SKILL.md path and the entries it owns, with paths
/// relative to the skill directory.
#[derive(Debug)]
pub struct Found {
    pub dir: String,
    pub entries: Vec<(String, TreeEntry)>,
}
/// Every SKILL.md under `root` (a directory prefix, empty for the whole tree) becomes a skill.
pub fn discover(tree: &[TreeEntry], root: &str) -> Result<Vec<Found>, Error> {
    let root = root.trim_matches('/');
    let within =
        |path: &str| root.is_empty() || path == root || path.starts_with(&format!("{root}/"));
    let dirs: Vec<String> = tree
        .iter()
        .filter(|e| within(&e.path))
        .filter_map(|e| {
            if e.path == ENTRYPOINT {
                Some(String::new())
            } else {
                e.path
                    .strip_suffix(&format!("/{ENTRYPOINT}"))
                    .map(str::to_owned)
            }
        })
        .collect();
    let mut found = Vec::new();
    for dir in &dirs {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        // Files under a deeper skill belong to that skill.
        let nested: Vec<String> = dirs
            .iter()
            .filter(|d| *d != dir && d.starts_with(&prefix))
            .map(|d| format!("{d}/"))
            .collect();
        let mut entries = Vec::new();
        for entry in tree {
            let Some(relative) = entry.path.strip_prefix(&prefix) else {
                continue;
            };
            if nested.iter().any(|n| entry.path.starts_with(n)) {
                continue;
            }
            if entry.symlink {
                return Err(Error::Invalid(format!(
                    "{} is a symlink; skills cannot contain symlinks",
                    entry.path
                )));
            }
            entries.push((relative.to_owned(), entry.clone()));
        }
        if entries.len() > MAX_FILES {
            return Err(Error::Invalid(format!(
                "The skill at {} has more than {MAX_FILES} files",
                if dir.is_empty() { "the root" } else { dir }
            )));
        }
        found.push(Found {
            dir: dir.clone(),
            entries,
        });
    }
    found.sort_by(|a, b| a.dir.cmp(&b.dir));
    Ok(found)
}
/// Names from front matter, falling back to the directory name; repeats get a numeric suffix.
pub fn assign_names(candidates: Vec<(String, String)>) -> Vec<String> {
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    candidates
        .into_iter()
        .map(|(declared, dir)| {
            let base = [
                declared,
                dir.rsplit('/').next().unwrap_or_default().to_owned(),
            ]
            .into_iter()
            .map(|v| normalize_name(&v))
            .find(|v| valid_name(v))
            .unwrap_or_else(|| "skill".into());
            let count = used.entry(base.clone()).or_default();
            *count += 1;
            if *count == 1 {
                base
            } else {
                let suffix = format!("-{count}");
                format!("{}{suffix}", &base[..base.len().min(64 - suffix.len())])
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn front_matter_reads_block_and_wrapped_scalars() {
        let folded = "---\nname: gui\ndescription: >-\n  Drive a desktop\n  through screenshots.\nlicense: MIT\n---\nbody";
        assert_eq!(
            front_matter(folded),
            (
                Some("gui".into()),
                "Drive a desktop through screenshots.".into()
            )
        );
        let wrapped = "---\nname: \"notes\"\ndescription: Take notes\n  in meetings.\n---\n";
        assert_eq!(front_matter(wrapped).1, "Take notes in meetings.");
    }

    use super::*;
    fn entry(path: &str) -> TreeEntry {
        TreeEntry {
            path: path.into(),
            size: 1,
            executable: false,
            symlink: false,
            id: path.into(),
        }
    }
    #[test]
    fn nested_skills_own_their_own_files_and_the_root_narrows_the_scan() {
        let tree = [
            "README.md",
            "skills/pdf/SKILL.md",
            "skills/pdf/forms.md",
            "skills/pdf/scripts/fill.py",
            "skills/pdf/extra/SKILL.md",
            "skills/pdf/extra/notes.md",
            "other/SKILL.md",
        ]
        .map(entry);
        let found = discover(&tree, "skills").unwrap();
        let paths = |f: &Found| f.entries.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].dir, "skills/pdf");
        assert_eq!(
            paths(&found[0]),
            ["SKILL.md", "forms.md", "scripts/fill.py"]
        );
        assert_eq!(paths(&found[1]), ["SKILL.md", "notes.md"]);
        let mut linked = entry("skills/pdf/link");
        linked.symlink = true;
        assert!(discover(&[entry("skills/pdf/SKILL.md"), linked], "").is_err());
    }
    #[test]
    fn names_come_from_front_matter_then_directory_and_repeats_are_suffixed() {
        assert_eq!(
            assign_names(vec![
                ("PDF Tools".into(), "a/pdf".into()),
                (String::new(), "b/pdf-tools".into()),
                (String::new(), "Weird Dir!".into()),
            ]),
            ["pdf-tools", "pdf-tools-2", "weird-dir"]
        );
        assert_eq!(
            front_matter("---\nname: x\ndescription: \"Do it\"\n---\n# X"),
            (Some("x".into()), "Do it".into())
        );
    }
    #[test]
    fn packages_validate_paths_sizes_and_hash_content_in_path_order() {
        let a = vec![
            File::new("b.md".into(), b"2".to_vec(), false),
            File::new(
                "SKILL.md".into(),
                b"---\ndescription: D\n---".to_vec(),
                false,
            ),
        ];
        let mut b = a.clone();
        b.reverse();
        let (a, b) = (
            Package::new("x".into(), String::new(), a).unwrap(),
            Package::new("x".into(), String::new(), b).unwrap(),
        );
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.description, "D");
        let binary = File::new("logo.png".into(), vec![0xff, 0xfe], false);
        assert!(binary.needs_object() && binary.media_type == "image/png");
        assert!(
            Package::new(
                "x".into(),
                String::new(),
                vec![File::new("../x".into(), vec![], false)]
            )
            .is_err()
        );
        assert!(
            Package::new(
                "x".into(),
                String::new(),
                vec![File::new("notes.md".into(), vec![], false)]
            )
            .is_err()
        );
    }
    #[test]
    fn a_skill_md_too_large_to_keep_inline_still_yields_its_description() {
        let mut md = b"---\ndescription: Big\n---\n".to_vec();
        md.resize(300 * 1024, b'x');
        let file = File::new(ENTRYPOINT.into(), md, false);
        assert!(file.needs_object());
        let package = Package::new("big".into(), String::new(), vec![file]).unwrap();
        assert_eq!(package.description, "Big");
    }
}
