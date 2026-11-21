//! The agent's skills in the harness's own skill folder (`~/.claude/skills`, `~/.codex/skills`),
//! where the harness finds them like any skill a person installed. A skill is a directory
//! named after it; the ones written here carry a `.tilde` file holding the version installed,
//! so a sync rewrites only skills that changed, removes the ones the agent no longer has, and
//! never touches a skill somebody else put there.
use super::{Connection, install};
use crate::api::{failed, options};
use anyhow::{Context, Result};
use std::path::Path;
use tilde_contracts::proto::tilde::local::v1 as wire;

const MARKER: &str = ".tilde";

/// Bring `root`, a harness's skill folder, up to date with the agent's skills.
pub async fn sync(connection: &Connection, root: &Path) -> Result<()> {
    let client = connection.client()?;
    let listed = client
        .list_skills_with_options(wire::ListSkillsRequest::default(), options())
        .await
        .map_err(|error| failed("ListSkills", error))?
        .into_owned()
        .skills;
    let mut kept = std::collections::BTreeSet::new();
    for skill in &listed {
        // The name becomes a directory: names are lowercase slugs, and two sources sharing one
        // install the first.
        if !install::plain_name(&skill.name) || !kept.insert(skill.name.clone()) {
            continue;
        }
        let dir = root.join(&skill.name);
        let installed = std::fs::read_to_string(dir.join(MARKER)).ok();
        if installed.as_deref() == Some(skill.version_id.as_str()) {
            continue;
        }
        if dir.exists() && installed.is_none() {
            eprintln!(
                "tilde local: {} already holds a skill named {}; leaving it",
                root.display(),
                skill.name
            );
            continue;
        }
        // Written beside the folder and moved in whole, so a failed download leaves the
        // installed version in place.
        let staging = root.join(format!(".{}.tilde-sync", skill.name));
        let _ = std::fs::remove_dir_all(&staging);
        for file in &skill.files {
            let read = client
                .read_skill_file_with_options(
                    wire::ReadSkillFileRequest {
                        name: format!("{}/{}", skill.source, skill.name),
                        path: file.path.clone(),
                        ..Default::default()
                    },
                    options(),
                )
                .await
                .map_err(|error| failed("ReadSkillFile", error))?
                .into_owned();
            let bytes = if read.download_url.is_empty() {
                read.content.into_bytes()
            } else {
                reqwest::get(&read.download_url)
                    .await
                    .and_then(|response| response.error_for_status())
                    .context("Could not download a skill file")?
                    .bytes()
                    .await?
                    .to_vec()
            };
            write(&staging, &file.path, &bytes, file.executable)?;
        }
        std::fs::write(staging.join(MARKER), &skill.version_id)?;
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(&staging, &dir)?;
    }
    // Skills this command installed and the agent no longer has.
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !kept.contains(&name) && entry.path().join(MARKER).is_file() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    Ok(())
}

/// One skill file below `dir`. The engine only stores safe relative paths; this refuses any
/// other all the same, since the path becomes one on this machine.
fn write(dir: &Path, relative: &str, bytes: &[u8], executable: bool) -> Result<()> {
    let safe = Path::new(relative)
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)));
    anyhow::ensure!(
        safe && !relative.is_empty(),
        "Unsafe skill file path {relative}"
    );
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().context("skill file directory")?)?;
    std::fs::write(&path, bytes)?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}
