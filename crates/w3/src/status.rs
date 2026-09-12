use crate::Error;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Status {
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicted: bool,
    pub upstream: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
}

impl Status {
    pub fn dirty(&self) -> bool {
        self.staged || self.unstaged || self.untracked || self.conflicted
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Status, Error> {
    let mut status = Status::default();
    let mut records = bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());
    while let Some(record) = records.next() {
        if let Some(upstream) = record.strip_prefix(b"# branch.upstream ") {
            status.upstream = Some(String::from_utf8_lossy(upstream).into_owned());
        } else if let Some(counts) = record.strip_prefix(b"# branch.ab ") {
            let counts = String::from_utf8_lossy(counts);
            let Some((ahead, behind)) = counts.split_once(' ') else {
                return Err(Error::Parse(counts.into_owned()));
            };
            status.ahead = ahead.strip_prefix('+').and_then(|n| n.parse().ok());
            status.behind = behind.strip_prefix('-').and_then(|n| n.parse().ok());
            if status.ahead.is_none() || status.behind.is_none() {
                return Err(Error::Parse(counts.into_owned()));
            }
        } else if record.starts_with(b"? ") {
            status.untracked = true;
        } else if record.starts_with(b"u ") {
            status.conflicted = true;
        } else if record.starts_with(b"1 ") || record.starts_with(b"2 ") {
            if record.len() < 5 || record[4] != b' ' {
                return Err(Error::Parse(String::from_utf8_lossy(record).into_owned()));
            }
            status.staged |= record[2] != b'.';
            status.unstaged |= record[3] != b'.';
            if record[0] == b'2' && records.next().is_none() {
                return Err(Error::Parse("rename record has no original path".into()));
            }
        } else if !record.starts_with(b"# ") {
            return Err(Error::Parse(String::from_utf8_lossy(record).into_owned()));
        }
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_tracks_dirty_flags_and_divergence() {
        let status = parse(
            b"# branch.upstream origin/main\0# branch.ab +2 -3\0\
1 MM N... 100644 100644 100644 abc abc file\0? new\0u UU conflict\0",
        )
        .unwrap();
        assert!(
            status.dirty()
                && status.staged
                && status.unstaged
                && status.untracked
                && status.conflicted
        );
        assert_eq!(status.upstream.as_deref(), Some("origin/main"));
        assert_eq!((status.ahead, status.behind), (Some(2), Some(3)));
    }

    #[test]
    fn rename_source_is_not_a_status_record() {
        let status =
            parse(b"2 R. N... 100644 100644 100644 abc abc R100 new\0? misleading\0").unwrap();
        assert!(status.staged);
        assert!(!status.untracked);
    }

    #[test]
    fn detached_or_missing_upstream_is_not_zero_divergence() {
        let status = parse(b"# branch.head (detached)\0").unwrap();
        assert!(!status.dirty());
        assert_eq!(
            (status.upstream, status.ahead, status.behind),
            (None, None, None)
        );
        assert!(parse(b"# branch.ab nope\0").is_err());
        assert!(parse(b"2 R. N... new\0").is_err());
    }
}
