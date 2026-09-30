//! The `apply_patch` format sandbox providers accept, parsed into hunks and applied to file
//! contents in memory. File I/O belongs to each provider. Ported from openai/codex's apply-patch
//! (Apache 2.0) by way of tilde-api; the grammar (lenient: surrounding whitespace and a heredoc
//! wrapper are accepted):
//!
//! ```text
//! *** Begin Patch
//! *** Add File: path        followed by "+" lines
//! *** Delete File: path
//! *** Update File: path     optionally "*** Move to: path", then chunks of
//!                           "@@ context" and " ", "+", "-" lines, optionally "*** End of File"
//! *** End Patch
//! ```
use crate::chat::tools::ToolResult;
use connectrpc::ConnectError;

#[derive(Debug, PartialEq)]
pub enum Hunk {
    Add {
        path: String,
        contents: String,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        move_to: Option<String>,
        chunks: Vec<Chunk>,
    },
}
#[derive(Debug, PartialEq)]
pub struct Chunk {
    /// A line (typically a function signature) the old lines must follow.
    context: Option<String>,
    old: Vec<String>,
    new: Vec<String>,
    end_of_file: bool,
}

fn invalid(message: String) -> ConnectError {
    ConnectError::invalid_argument(format!("Invalid patch: {message}"))
}

pub fn parse(patch: &str) -> ToolResult<Vec<Hunk>> {
    let mut lines: Vec<&str> = patch.trim().lines().collect();
    if lines.len() >= 4
        && ["<<EOF", "<<'EOF'", "<<\"EOF\""].contains(&lines[0])
        && lines[lines.len() - 1].ends_with("EOF")
    {
        lines = lines[1..lines.len() - 1].to_vec();
    }
    if lines.first().map(|l| l.trim()) != Some("*** Begin Patch") {
        return Err(invalid("the first line must be '*** Begin Patch'".into()));
    }
    if lines.len() < 2 || lines[lines.len() - 1].trim() != "*** End Patch" {
        return Err(invalid("the last line must be '*** End Patch'".into()));
    }
    let mut rest = &lines[1..lines.len() - 1];
    let mut hunks = Vec::new();
    let mut number = 2;
    while !rest.is_empty() {
        let (hunk, used) = hunk(rest, number)?;
        hunks.push(hunk);
        number += used;
        rest = &rest[used..];
    }
    if hunks.is_empty() {
        return Err(invalid("no hunks".into()));
    }
    Ok(hunks)
}

fn hunk(lines: &[&str], number: usize) -> ToolResult<(Hunk, usize)> {
    let first = lines[0].trim();
    if let Some(path) = first.strip_prefix("*** Add File: ") {
        let added: Vec<&str> = lines[1..]
            .iter()
            .map_while(|line| line.strip_prefix('+'))
            .collect();
        let contents = added.iter().map(|line| format!("{line}\n")).collect();
        return Ok((
            Hunk::Add {
                path: path.into(),
                contents,
            },
            added.len() + 1,
        ));
    }
    if let Some(path) = first.strip_prefix("*** Delete File: ") {
        return Ok((Hunk::Delete { path: path.into() }, 1));
    }
    let Some(path) = first.strip_prefix("*** Update File: ") else {
        return Err(invalid(format!(
            "line {number}: '{first}' is not a hunk header; use '*** Add File: ', '*** Delete File: ' or '*** Update File: '"
        )));
    };
    let mut used = 1;
    let move_to = lines
        .get(1)
        .and_then(|line| line.strip_prefix("*** Move to: "));
    if move_to.is_some() {
        used += 1;
    }
    let mut chunks = Vec::new();
    while used < lines.len() {
        if lines[used].trim().is_empty() {
            used += 1;
            continue;
        }
        if lines[used].starts_with('*') {
            break;
        }
        let (chunk, n) = chunk(&lines[used..], number + used, chunks.is_empty())?;
        chunks.push(chunk);
        used += n;
    }
    if chunks.is_empty() {
        return Err(invalid(format!(
            "line {number}: the update of '{path}' is empty"
        )));
    }
    Ok((
        Hunk::Update {
            path: path.into(),
            move_to: move_to.map(Into::into),
            chunks,
        },
        used,
    ))
}

fn chunk(lines: &[&str], number: usize, first: bool) -> ToolResult<(Chunk, usize)> {
    let (context, start) = if lines[0] == "@@" {
        (None, 1)
    } else if let Some(context) = lines[0].strip_prefix("@@ ") {
        (Some(context.to_owned()), 1)
    } else if first {
        (None, 0)
    } else {
        return Err(invalid(format!(
            "line {number}: expected an '@@' context marker, got '{}'",
            lines[0]
        )));
    };
    let mut chunk = Chunk {
        context,
        old: vec![],
        new: vec![],
        end_of_file: false,
    };
    let mut used = 0;
    for line in &lines[start..] {
        if *line == "*** End of File" {
            if used == 0 {
                break;
            }
            chunk.end_of_file = true;
            used += 1;
            break;
        }
        match line.chars().next() {
            None => {
                chunk.old.push(String::new());
                chunk.new.push(String::new());
            }
            Some(' ') => {
                chunk.old.push(line[1..].into());
                chunk.new.push(line[1..].into());
            }
            Some('+') => chunk.new.push(line[1..].into()),
            Some('-') => chunk.old.push(line[1..].into()),
            // The next hunk or chunk.
            _ if used > 0 => break,
            _ => {
                return Err(invalid(format!(
                    "line {}: '{line}' must start with ' ', '+' or '-'",
                    number + start
                )));
            }
        }
        used += 1;
    }
    if used == 0 {
        return Err(invalid(format!("line {number}: the chunk has no lines")));
    }
    Ok((chunk, used + start))
}

/// Apply an update's chunks to a file's contents; the result ends with a newline.
pub fn update(original: &str, path: &str, chunks: &[Chunk]) -> ToolResult<String> {
    let mut lines: Vec<String> = original.split('\n').map(String::from).collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut from = 0;
    for chunk in chunks {
        if let Some(context) = &chunk.context {
            from = seek(&lines, std::slice::from_ref(context), from, false)
                .ok_or_else(|| invalid(format!("context '{context}' not found in {path}")))?
                + 1;
        }
        if chunk.old.is_empty() {
            replacements.push((lines.len(), 0, chunk.new.clone()));
            continue;
        }
        let (mut old, mut new) = (&chunk.old[..], &chunk.new[..]);
        let mut found = seek(&lines, old, from, chunk.end_of_file);
        // A trailing empty old line stands for the file's final newline, which `lines` lacks.
        if found.is_none() && old.last().is_some_and(String::is_empty) {
            old = &old[..old.len() - 1];
            if new.last().is_some_and(String::is_empty) {
                new = &new[..new.len() - 1];
            }
            found = seek(&lines, old, from, chunk.end_of_file);
        }
        let start = found.ok_or_else(|| {
            invalid(format!(
                "expected lines not found in {path}:\n{}",
                chunk.old.join("\n")
            ))
        })?;
        replacements.push((start, old.len(), new.to_vec()));
        from = start + old.len();
    }
    replacements.sort_by_key(|(start, _, _)| *start);
    for (start, len, new) in replacements.into_iter().rev() {
        lines.splice(start..(start + len).min(lines.len()), new);
    }
    if !lines.last().is_some_and(String::is_empty) {
        lines.push(String::new());
    }
    Ok(lines.join("\n"))
}

/// Find `pattern` in `lines` at or after `start`, with decreasing strictness: exact, then
/// ignoring trailing whitespace, then surrounding whitespace, then typographic punctuation.
/// `eof` tries the end of the file first.
fn seek(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }
    let first = if eof {
        lines.len() - pattern.len()
    } else {
        start
    };
    let normalise = |s: &str| -> String {
        s.trim()
            .chars()
            .map(|c| match c {
                '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
                '\u{2018}'..='\u{201B}' => '\'',
                '\u{201C}'..='\u{201F}' => '"',
                '\u{00A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => ' ',
                other => other,
            })
            .collect()
    };
    let same = |pass: u8, a: &str, b: &str| match pass {
        0 => a == b,
        1 => a.trim_end() == b.trim_end(),
        2 => a.trim() == b.trim(),
        _ => normalise(a) == normalise(b),
    };
    (0..4).find_map(|pass| {
        (first..=lines.len() - pattern.len()).find(|&i| {
            pattern
                .iter()
                .enumerate()
                .all(|(j, p)| same(pass, &lines[i + j], p))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_apply_after_context_with_moves_and_lenient_whitespace() {
        let hunks = parse(
            "<<'EOF'\n*** Begin Patch\n*** Add File: notes.txt\n+hello\n*** Update File: src/lib.py\n*** Move to: src/main.py\n@@ def two():\n-    return 1  \n+    return 2\n*** Delete File: old.txt\n*** End Patch\nEOF",
        )
        .unwrap();
        assert_eq!(
            hunks[0],
            Hunk::Add {
                path: "notes.txt".into(),
                contents: "hello\n".into()
            }
        );
        let Hunk::Update {
            move_to, chunks, ..
        } = &hunks[1]
        else {
            panic!("{hunks:?}")
        };
        assert_eq!(move_to.as_deref(), Some("src/main.py"));
        let original = "def one():\n    return 1\ndef two():\n    return 1\n";
        assert_eq!(
            update(original, "src/lib.py", chunks).unwrap(),
            "def one():\n    return 1\ndef two():\n    return 2\n",
            "the context skips the first match; trailing whitespace is forgiven"
        );
        assert_eq!(
            hunks[2],
            Hunk::Delete {
                path: "old.txt".into()
            }
        );
        assert!(update("nothing here\n", "x", chunks).is_err());
        assert!(parse("*** Begin Patch\n*** Rename: a\n*** End Patch").is_err());
    }
}
