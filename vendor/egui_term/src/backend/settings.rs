use std::path::PathBuf;

use alacritty_terminal::vte::ansi::CursorShape;

const DEFAULT_SHELL: &str = "/bin/bash";

#[derive(Debug, Clone)]
pub struct BackendSettings {
    pub shell: String,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    /// The cursor shape/blink to start with. An app that sets its own via
    /// DECSCUSR (vim's insert-mode beam, say) overrides this at runtime —
    /// it's only the default before anything has.
    pub cursor_shape: CursorShape,
    pub cursor_blinking: bool,
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            shell: DEFAULT_SHELL.to_string(),
            args: vec![],
            working_directory: None,
            cursor_shape: CursorShape::Block,
            cursor_blinking: true,
        }
    }
}
