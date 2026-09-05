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
    /// Allow terminal apps to *read* the system clipboard via OSC 52
    /// (`\e]52;c;?\a`). Off by default, matching Alacritty — only the copy
    /// direction is always permitted.
    pub osc52_read: bool,
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            shell: DEFAULT_SHELL.to_string(),
            args: vec![],
            working_directory: None,
            cursor_shape: CursorShape::Block,
            cursor_blinking: true,
            osc52_read: false,
        }
    }
}
