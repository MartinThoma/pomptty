//! Small helpers shared across the chrome widgets.

/// Rewrite a leading home directory in an absolute path as `~`.
///
/// Resolves the home directory via `directories::BaseDirs` (the same crate as
/// the config/session/history paths) rather than reading `$HOME` directly, so
/// it's also correct on Windows.
pub fn collapse_home(path: &str) -> String {
    let home = directories::BaseDirs::new().and_then(|d| d.home_dir().to_str().map(str::to_owned));
    collapse(path, home.as_deref())
}

fn collapse(path: &str, home: Option<&str>) -> String {
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return path.to_owned();
    };
    if path == home {
        return "~".to_owned();
    }
    match path.strip_prefix(&format!("{home}/")) {
        Some(rest) => format!("~/{rest}"),
        None => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::collapse;

    #[test]
    fn collapse_home_rewrites_the_prefix() {
        let home = Some("/home/tester");
        assert_eq!(collapse("/home/tester", home), "~");
        assert_eq!(collapse("/home/tester/src/main.rs", home), "~/src/main.rs");
        assert_eq!(collapse("/etc/hosts", home), "/etc/hosts");
        assert_eq!(collapse("/x", None), "/x");
        assert_eq!(collapse("/x", Some("")), "/x");
    }
}
