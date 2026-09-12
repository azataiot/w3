use crate::Error;

pub fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::InvalidName("name must not be empty".into()));
    }
    if name.starts_with('-') {
        return Err(Error::InvalidName("name must not start with -".into()));
    }
    if let Some(found) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/')))
    {
        return Err(Error::InvalidName(format!(
            "name must use letters, digits, -, _, and /: found '{found}'"
        )));
    }
    if name.split('/').any(str::is_empty) {
        return Err(Error::InvalidName(
            "name must not contain an empty slash-separated component".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_names() {
        for name in ["feat", "Feat9", "a-b", "a_b", "a/b/c"] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_invalid_names() {
        for name in [
            "",
            "-x",
            "/",
            "a//b",
            "a/",
            "/a",
            "release.1",
            "feat(api)",
            "für",
        ] {
            assert!(
                matches!(validate_name(name), Err(Error::InvalidName(_))),
                "{name}"
            );
        }
    }
}
