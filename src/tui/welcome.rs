use crossterm::event::{KeyCode, KeyModifiers};
use std::path::PathBuf;

/// Which field in the welcome form is currently focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedField {
    PathInput,
    LangSelector,
    ExcludeInput,
    ConfirmButton,
}

impl FocusedField {
    pub fn next(self) -> Self {
        match self {
            FocusedField::PathInput => FocusedField::LangSelector,
            FocusedField::LangSelector => FocusedField::ExcludeInput,
            FocusedField::ExcludeInput => FocusedField::ConfirmButton,
            FocusedField::ConfirmButton => FocusedField::PathInput,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            FocusedField::PathInput => FocusedField::ConfirmButton,
            FocusedField::LangSelector => FocusedField::PathInput,
            FocusedField::ExcludeInput => FocusedField::LangSelector,
            FocusedField::ConfirmButton => FocusedField::ExcludeInput,
        }
    }
}

/// Language filter options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangOption {
    All,
    Rust,
    JavaBased,
    JsTs,
    Python,
}

impl LangOption {
    pub const ALL_OPTIONS: &'static [LangOption] = &[
        LangOption::All,
        LangOption::Rust,
        LangOption::JavaBased,
        LangOption::JsTs,
        LangOption::Python,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LangOption::All => "All",
            LangOption::Rust => "Rust",
            LangOption::JavaBased => "Java Based",
            LangOption::JsTs => "JS/TS",
            LangOption::Python => "Python",
        }
    }
}

/// Result of the welcome screen when the user confirms.
pub struct WelcomeConfig {
    pub path: PathBuf,
    pub language: LangOption,
    pub exclude_patterns: Vec<String>,
}

/// State of the welcome/input screen.
pub struct WelcomeApp {
    pub path_input: String,
    pub lang_index: usize,
    pub exclude_input: String,
    pub focused: FocusedField,
    pub confirmed: bool,
    pub should_quit: bool,
    pub error_msg: Option<String>,
}

impl WelcomeApp {
    pub fn new() -> Self {
        WelcomeApp {
            path_input: String::new(),
            lang_index: 0,
            exclude_input: String::new(),
            focused: FocusedField::PathInput,
            confirmed: false,
            should_quit: false,
            error_msg: None,
        }
    }

    pub fn selected_lang(&self) -> LangOption {
        LangOption::ALL_OPTIONS[self.lang_index]
    }

    pub fn handle_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) {
        // Clear previous error on any key
        self.error_msg = None;

        match key {
            KeyCode::Esc => self.should_quit = true,

            // Navigation between fields
            KeyCode::Tab | KeyCode::Down => {
                self.focused = self.focused.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = self.focused.prev();
            }

            // Enter: confirm button → submit; text fields → move to next
            KeyCode::Enter => {
                if self.focused == FocusedField::ConfirmButton {
                    self.try_confirm();
                } else {
                    self.focused = self.focused.next();
                }
            }

            // Left/Right: cycle language selector
            KeyCode::Left => {
                if self.focused == FocusedField::LangSelector {
                    if self.lang_index > 0 {
                        self.lang_index -= 1;
                    } else {
                        self.lang_index = LangOption::ALL_OPTIONS.len() - 1;
                    }
                }
            }
            KeyCode::Right => {
                if self.focused == FocusedField::LangSelector {
                    self.lang_index =
                        (self.lang_index + 1) % LangOption::ALL_OPTIONS.len();
                }
            }

            // Backspace: delete last char from text inputs
            KeyCode::Backspace => match self.focused {
                FocusedField::PathInput => {
                    self.path_input.pop();
                }
                FocusedField::ExcludeInput => {
                    self.exclude_input.pop();
                }
                _ => {}
            },

            // Character input
            KeyCode::Char(c) => match self.focused {
                FocusedField::PathInput => self.path_input.push(c),
                FocusedField::ExcludeInput => self.exclude_input.push(c),
                FocusedField::LangSelector => match c {
                    // vim-style navigation in language selector
                    'h' => {
                        if self.lang_index > 0 {
                            self.lang_index -= 1;
                        } else {
                            self.lang_index = LangOption::ALL_OPTIONS.len() - 1;
                        }
                    }
                    'l' => {
                        self.lang_index =
                            (self.lang_index + 1) % LangOption::ALL_OPTIONS.len();
                    }
                    _ => {}
                },
                FocusedField::ConfirmButton => {}
            },

            _ => {}
        }
    }

    /// Validate inputs and set `confirmed = true` if valid.
    fn try_confirm(&mut self) {
        let path_str = self.path_input.trim().to_string();
        if path_str.is_empty() {
            self.error_msg = Some("⚠  Project path cannot be empty.".to_string());
            self.focused = FocusedField::PathInput;
            return;
        }
        let path = PathBuf::from(&path_str);
        if !path.exists() {
            self.error_msg = Some(format!("⚠  Path does not exist: {}", path_str));
            self.focused = FocusedField::PathInput;
            return;
        }
        if !path.is_dir() {
            self.error_msg = Some(format!("⚠  Not a directory: {}", path_str));
            self.focused = FocusedField::PathInput;
            return;
        }
        self.confirmed = true;
    }

    /// Consume self and produce the confirmed config.
    pub fn into_config(self) -> WelcomeConfig {
        let exclude_patterns: Vec<String> = self
            .exclude_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        WelcomeConfig {
            path: PathBuf::from(self.path_input.trim()),
            language: self.selected_lang(),
            exclude_patterns,
        }
    }
}
