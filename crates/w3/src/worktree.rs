use std::path::PathBuf;

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub locked: Option<String>,
    pub prunable: Option<String>,
    pub bare: bool,
}

pub fn parse_porcelain(bytes: &[u8]) -> Result<Vec<Worktree>, Error> {
    let mut worktrees = Vec::new();
    let mut current: Option<Worktree> = None;
    for line in bytes.split(|byte| *byte == 0) {
        if line.is_empty() {
            worktrees.extend(current.take());
            continue;
        }
        let (keyword, value) = split_keyword(line);
        if keyword == b"worktree" {
            worktrees.extend(current.take());
            current = Some(Worktree {
                path: path_from_bytes(value),
                head: String::new(),
                branch: None,
                locked: None,
                prunable: None,
                bare: false,
            });
            continue;
        }
        let Some(worktree) = current.as_mut() else {
            return Err(Error::Parse(String::from_utf8_lossy(line).into_owned()));
        };
        let text = String::from_utf8_lossy(value).into_owned();
        match keyword {
            b"HEAD" => worktree.head = text,
            b"branch" => {
                worktree.branch = Some(
                    text.strip_prefix("refs/heads/")
                        .unwrap_or(&text)
                        .to_string(),
                )
            }
            b"detached" => worktree.branch = None,
            b"locked" => worktree.locked = Some(text),
            b"prunable" => worktree.prunable = Some(text),
            b"bare" => worktree.bare = true,
            _ => {}
        }
    }
    worktrees.extend(current.take());
    Ok(worktrees)
}

fn split_keyword(line: &[u8]) -> (&[u8], &[u8]) {
    match line.iter().position(|byte| *byte == b' ') {
        Some(index) => (&line[..index], &line[index + 1..]),
        None => (line, &[]),
    }
}

#[cfg(unix)]
pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "worktree /home/dev/project\0\
HEAD 1111111111111111111111111111111111111111\0\
branch refs/heads/main\0\
\0\
worktree /home/dev/.worktrees/project/feature-a\0\
HEAD 2222222222222222222222222222222222222222\0\
branch refs/heads/feature-a\0\
locked {\"owner\":\"agent\",\"createdAt\":1700000000}\0\
\0\
worktree /home/dev/.worktrees/project/detached-b\0\
HEAD 3333333333333333333333333333333333333333\0\
detached\0\
locked {\"owner\":\"agent\",\"createdAt\":1700000000}\0\
\0\
";

    const AGENT_LOCK: &str = "{\"owner\":\"agent\",\"createdAt\":1700000000}";

    #[test]
    fn parses_three_records() {
        let worktrees = parse_porcelain(SAMPLE.as_bytes()).unwrap();
        assert_eq!(worktrees.len(), 3);
    }

    #[test]
    fn main_worktree_has_branch_and_no_lock() {
        let worktrees = parse_porcelain(SAMPLE.as_bytes()).unwrap();
        assert_eq!(
            worktrees[0],
            Worktree {
                path: PathBuf::from("/home/dev/project"),
                head: "1111111111111111111111111111111111111111".into(),
                branch: Some("main".into()),
                locked: None,
                prunable: None,
                bare: false,
            }
        );
    }

    #[test]
    fn locked_branch_keeps_json_reason_verbatim() {
        let worktrees = parse_porcelain(SAMPLE.as_bytes()).unwrap();
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature-a"));
        assert_eq!(worktrees[1].locked.as_deref(), Some(AGENT_LOCK));
    }

    #[test]
    fn detached_head_has_no_branch() {
        let worktrees = parse_porcelain(SAMPLE.as_bytes()).unwrap();
        assert_eq!(worktrees[2].branch, None);
        assert_eq!(
            worktrees[2].head,
            "3333333333333333333333333333333333333333"
        );
        assert_eq!(worktrees[2].locked.as_deref(), Some(AGENT_LOCK));
    }

    #[test]
    fn keyword_without_reason_is_some_empty() {
        let sample = "worktree /tmp/a\0HEAD abc\0branch refs/heads/x\0locked\0\0\
worktree /tmp/b\0HEAD abc\0detached\0prunable gitdir file points to non-existent location\0\0\
worktree /tmp/c\0HEAD abc\0bare\0\0";
        let worktrees = parse_porcelain(sample.as_bytes()).unwrap();
        assert_eq!(worktrees[0].locked.as_deref(), Some(""));
        assert_eq!(
            worktrees[1].prunable.as_deref(),
            Some("gitdir file points to non-existent location")
        );
        assert!(worktrees[2].bare);
    }

    #[test]
    fn empty_input_is_empty_list() {
        assert_eq!(parse_porcelain(b"").unwrap(), Vec::<Worktree>::new());
    }

    #[test]
    fn field_before_worktree_header_is_an_error() {
        let err = parse_porcelain(b"HEAD abc\0\0").unwrap_err();
        assert!(matches!(err, Error::Parse(_)));
    }
}
