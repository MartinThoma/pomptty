//! Small helpers shared across the chrome widgets.

/// Rewrite a leading home directory in an absolute path as `~`.
///
/// Uses `directories::BaseDirs` (same crate as the config/session/history
/// paths elsewhere) rather than reading `$HOME` directly, since that's unset
/// on native Windows — `BaseDirs` resolves the right thing on every platform.
pub fn collapse_home(path: &str) -> String {
    if let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
        && let Some(home) = home.to_str()
        && !home.is_empty()
    {
        if path == home {
            return "~".to_owned();
        }
        if let Some(rest) = path.strip_prefix(&format!("{home}/")) {
            return format!("~/{rest}");
        }
    }
    path.to_owned()
}

#[cfg(test)]
mod tests {
    use super::collapse_home;

    #[test]
    fn collapse_home_rewrites_the_prefix() {
        // SAFETY: single-threaded test process.
        unsafe { std::env::set_var("HOME", "/home/tester") };
        assert_eq!(collapse_home("/home/tester"), "~");
        assert_eq!(collapse_home("/home/tester/src/main.rs"), "~/src/main.rs");
        assert_eq!(collapse_home("/etc/hosts"), "/etc/hosts");
    }
}
