//! Git smart-HTTP, as much as the egress gateway reads of it: which service a request is for,
//! the ref updates a push starts with, whether the pack it carries proves each update a
//! fast-forward, and the default branch an upload-pack advertisement names.
use crate::connections::egress::{GitService, RefUpdate, ZERO_SHA};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::io::Read;

/// A git smart-HTTP path: `<repository>.git/info/refs` (with `?service=`), or
/// `<repository>.git/git-upload-pack` / `git-receive-pack`. The repository is returned without
/// `.git`; `advertisement` is true for `info/refs`.
pub fn route<'p>(path: &'p str, query: Option<&str>) -> Option<(&'p str, GitService, bool)> {
    let path = path.strip_prefix('/')?;
    let (repository, service, advertisement) = match path.strip_suffix("/info/refs") {
        Some(repository) => {
            let service = query?
                .split('&')
                .find_map(|pair| pair.strip_prefix("service="))
                .and_then(GitService::parse)?;
            (repository, service, true)
        }
        None => {
            let (repository, service) = path.rsplit_once('/')?;
            (repository, GitService::parse(service)?, false)
        }
    };
    let repository = repository.strip_suffix(".git").unwrap_or(repository);
    // Plain path segments only: nothing that could name another repository upstream.
    let plain = repository.split('/').all(|segment| {
        !segment.trim_matches('.').is_empty()
            && segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    });
    plain.then_some((repository, service, advertisement))
}

/// One pkt-line: `Some(payload)` or `None` for a flush/delimiter packet, and the rest.
fn pkt_line(body: &[u8]) -> Result<(Option<&[u8]>, &[u8]), String> {
    let invalid = || "Malformed pkt-line".to_owned();
    let length = std::str::from_utf8(body.get(..4).ok_or_else(invalid)?)
        .ok()
        .and_then(|h| usize::from_str_radix(h, 16).ok())
        .ok_or_else(invalid)?;
    match length {
        0..=2 => Ok((None, &body[4..])),
        3 => Err(invalid()),
        _ => Ok((
            Some(body.get(4..length).ok_or_else(invalid)?),
            &body[length..],
        )),
    }
}

/// The ref updates a receive-pack request starts with, and the pack after them (and after the
/// push options, when the client sends any). The request is `*shallow command-list
/// [push-options] [pack]` (gitprotocol-pack): a shallow clone first names its shallow commits,
/// and capabilities ride on the first command. Signed pushes (`push-cert`) are not read; GitHub
/// does not advertise them. A bare flush, which git POSTs as a probe before a push larger than
/// `http.postBuffer`, names no updates.
pub fn commands(body: &[u8]) -> Result<(Vec<RefUpdate>, &[u8]), String> {
    let mut updates = vec![];
    let mut rest = body;
    let mut push_options = false;
    loop {
        let (line, next) = pkt_line(rest)?;
        rest = next;
        let Some(line) = line else { break };
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        if updates.is_empty() && line.starts_with(b"shallow ") {
            continue;
        }
        if updates.is_empty() && line.starts_with(b"push-cert\0") {
            return Err("Signed pushes are not supported".into());
        }
        let (command, capabilities) = match line.iter().position(|&b| b == 0) {
            Some(nul) => (&line[..nul], Some(&line[nul + 1..])),
            None => (line, None),
        };
        if let Some(capabilities) = capabilities {
            push_options = capabilities
                .split(|&b| b == b' ')
                .any(|c| c == b"push-options");
        }
        let command = std::str::from_utf8(command).map_err(|_| "Malformed ref update")?;
        let mut parts = command.splitn(3, ' ');
        let (Some(old), Some(new), Some(name)) = (parts.next(), parts.next(), parts.next()) else {
            return Err("Malformed ref update".into());
        };
        let sha = |s: &str| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if !sha(old) || !sha(new) || name.is_empty() {
            return Err("Malformed ref update".into());
        }
        updates.push(RefUpdate {
            old: old.to_ascii_lowercase(),
            new: new.to_ascii_lowercase(),
            name: name.to_owned(),
        });
    }
    if updates.is_empty() {
        return if rest.is_empty() {
            Ok((updates, rest))
        } else {
            Err("A push names no ref updates".into())
        };
    }
    if push_options {
        loop {
            let (line, next) = pkt_line(rest)?;
            rest = next;
            if line.is_none() {
                break;
            }
        }
    }
    Ok((updates, rest))
}

/// The default branch (`refs/heads/main`) an upload-pack advertisement names in its
/// `symref=HEAD:` capability.
pub fn default_branch(advertisement: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(advertisement);
    text.split(['\0', ' ', '\n'])
        .find_map(|c| c.strip_prefix("symref=HEAD:"))
        .map(str::to_owned)
}

/// For each update, whether the pack proves it a fast-forward: its new commit is old's
/// descendant through commits in the pack. Anything unprovable (a commit already upstream that
/// is not `old`, an unreadable or deltified-against-upstream object) reads as not proven.
pub fn fast_forwards(updates: &[RefUpdate], pack: &[u8]) -> Vec<bool> {
    let parents = commits(pack).unwrap_or_default();
    updates
        .iter()
        .map(|update| {
            if update.old == ZERO_SHA || update.new == ZERO_SHA {
                return false;
            }
            let mut seen = HashSet::new();
            let mut stack = vec![update.new.as_str()];
            while let Some(commit) = stack.pop() {
                if commit == update.old {
                    return true;
                }
                if !seen.insert(commit) {
                    continue;
                }
                if let Some(parents) = parents.get(commit) {
                    stack.extend(parents.iter().map(String::as_str));
                }
            }
            false
        })
        .collect()
}

/// Every commit in a packfile and its parents. Commits stored as offset deltas are resolved
/// against their bases in the pack; ref deltas (thin packs' bases upstream) are skipped.
fn commits(pack: &[u8]) -> Option<HashMap<String, Vec<String>>> {
    if pack.get(..4)? != b"PACK" {
        return None;
    }
    let count = u32::from_be_bytes(pack.get(8..12)?.try_into().ok()?) as usize;
    let mut position = 12;
    // Offset → (type, inflated data) of objects later deltas may refer to.
    let mut objects: HashMap<usize, (u8, Vec<u8>)> = HashMap::new();
    let mut parents = HashMap::new();
    for _ in 0..count {
        let start = position;
        let mut byte = *pack.get(position)?;
        position += 1;
        let kind = (byte >> 4) & 7;
        let mut size = (byte & 15) as usize;
        let mut shift = 4;
        while byte & 0x80 != 0 {
            byte = *pack.get(position)?;
            position += 1;
            size |= ((byte & 0x7f) as usize).checked_shl(shift)?;
            shift += 7;
        }
        let base = match kind {
            6 => {
                byte = *pack.get(position)?;
                position += 1;
                let mut offset = (byte & 0x7f) as usize;
                while byte & 0x80 != 0 {
                    byte = *pack.get(position)?;
                    position += 1;
                    offset = ((offset + 1) << 7) | (byte & 0x7f) as usize;
                }
                Some(start.checked_sub(offset)?)
            }
            7 => {
                position += 20;
                None
            }
            _ => None,
        };
        let (data, used) = inflate(pack.get(position..)?, size)?;
        position += used;
        let resolved = match (kind, base) {
            (1..=4, _) => Some((kind, data)),
            (6, Some(base)) => objects
                .get(&base)
                .and_then(|(kind, base)| Some((*kind, apply_delta(base, &data)?))),
            _ => None,
        };
        let Some((kind, data)) = resolved else {
            continue;
        };
        if kind == 1 {
            let mut hash = Sha1::new();
            hash.update(format!("commit {}\0", data.len()).as_bytes());
            hash.update(&data);
            parents.insert(hex::encode(hash.finalize()), commit_parents(&data));
            // Git deltifies objects only against objects of their own type.
            objects.insert(start, (kind, data));
        }
    }
    Some(parents)
}
fn inflate(input: &[u8], size: usize) -> Option<(Vec<u8>, usize)> {
    let mut decoder = flate2::read::ZlibDecoder::new(input);
    let mut out = Vec::with_capacity(size.min(1 << 20));
    decoder.read_to_end(&mut out).ok()?;
    (out.len() == size).then_some((out, decoder.total_in() as usize))
}
fn commit_parents(data: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(data)
        .lines()
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.strip_prefix("parent "))
        .map(|sha| sha.trim().to_ascii_lowercase())
        .collect()
}
/// Git's delta format: source and target sizes, then copy and insert instructions.
fn apply_delta(base: &[u8], delta: &[u8]) -> Option<Vec<u8>> {
    let mut position = 0;
    let mut varint = || {
        let mut value = 0usize;
        let mut shift = 0;
        loop {
            let byte = *delta.get(position)?;
            position += 1;
            value |= ((byte & 0x7f) as usize).checked_shl(shift)?;
            shift += 7;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
    };
    if varint()? != base.len() {
        return None;
    }
    let target = varint()?;
    let mut out = Vec::with_capacity(target.min(1 << 20));
    while position < delta.len() {
        let op = delta[position];
        position += 1;
        if op & 0x80 != 0 {
            let mut offset = 0usize;
            let mut length = 0usize;
            for i in 0..4 {
                if op & (1 << i) != 0 {
                    offset |= (*delta.get(position)? as usize) << (8 * i);
                    position += 1;
                }
            }
            for i in 0..3 {
                if op & (0x10 << i) != 0 {
                    length |= (*delta.get(position)? as usize) << (8 * i);
                    position += 1;
                }
            }
            if length == 0 {
                length = 0x10000;
            }
            out.extend_from_slice(base.get(offset..offset.checked_add(length)?)?);
        } else if op != 0 {
            out.extend_from_slice(delta.get(position..position + op as usize)?);
            position += op as usize;
        } else {
            return None;
        }
    }
    (out.len() == target).then_some(out)
}
