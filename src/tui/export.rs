use crossterm::event::KeyCode;

/// Export format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Machine,
    Vault,
    Both,
}

impl ExportFormat {
    pub const ALL: &'static [ExportFormat] =
        &[ExportFormat::Machine, ExportFormat::Vault, ExportFormat::Both];

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Machine => "Machine Context",
            ExportFormat::Vault => "Obsidian Vault (Directory)",
            ExportFormat::Both => "Both",
        }
    }

    pub fn default_filename(self) -> &'static str {
        match self {
            ExportFormat::Machine => "context.txt",
            ExportFormat::Vault => "my_vault",
            ExportFormat::Both => "output",
        }
    }
}

/// Which field in the export overlay is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportField {
    FormatSelector,
    PathInput,
    ExportButton,
}

impl ExportField {
    pub fn next(self) -> Self {
        match self {
            ExportField::FormatSelector => ExportField::PathInput,
            ExportField::PathInput => ExportField::ExportButton,
            ExportField::ExportButton => ExportField::FormatSelector,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            ExportField::FormatSelector => ExportField::ExportButton,
            ExportField::PathInput => ExportField::FormatSelector,
            ExportField::ExportButton => ExportField::PathInput,
        }
    }
}

/// Status after an export attempt.
#[derive(Debug, Clone)]
pub enum ExportStatus {
    Success(String),
    Error(String),
}

/// State of the export overlay.
pub struct ExportApp {
    pub format_index: usize,
    pub path_input: String,
    pub focused: ExportField,
    pub status: Option<ExportStatus>,
    pub should_close: bool,
    /// Set to true when the user requests an export — caller performs the I/O.
    pub do_export: bool,
}

impl ExportApp {
    pub fn new() -> Self {
        ExportApp {
            format_index: 0,
            path_input: ExportFormat::Machine.default_filename().to_string(),
            focused: ExportField::FormatSelector,
            status: None,
            should_close: false,
            do_export: false,
        }
    }

    pub fn selected_format(&self) -> ExportFormat {
        ExportFormat::ALL[self.format_index]
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => self.should_close = true,

            KeyCode::Tab | KeyCode::Down => {
                self.focused = self.focused.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = self.focused.prev();
            }

            KeyCode::Enter => {
                if self.focused == ExportField::ExportButton {
                    self.do_export = true;
                } else {
                    self.focused = self.focused.next();
                }
            }

            KeyCode::Left => {
                if self.focused == ExportField::FormatSelector {
                    if self.format_index > 0 {
                        self.format_index -= 1;
                    } else {
                        self.format_index = ExportFormat::ALL.len() - 1;
                    }
                    self.auto_update_path();
                }
            }
            KeyCode::Right => {
                if self.focused == ExportField::FormatSelector {
                    self.format_index = (self.format_index + 1) % ExportFormat::ALL.len();
                    self.auto_update_path();
                }
            }

            KeyCode::Backspace => {
                if self.focused == ExportField::PathInput {
                    self.path_input.pop();
                    self.status = None;
                }
            }

            KeyCode::Char(c) => match self.focused {
                ExportField::PathInput => {
                    self.path_input.push(c);
                    self.status = None;
                }
                ExportField::FormatSelector => match c {
                    'h' => {
                        if self.format_index > 0 {
                            self.format_index -= 1;
                        } else {
                            self.format_index = ExportFormat::ALL.len() - 1;
                        }
                        self.auto_update_path();
                    }
                    'l' => {
                        self.format_index = (self.format_index + 1) % ExportFormat::ALL.len();
                        self.auto_update_path();
                    }
                    _ => {}
                },
                ExportField::ExportButton => {}
            },

            _ => {}
        }
    }

    /// If the path is still a known default filename, replace it with the new default.
    fn auto_update_path(&mut self) {
        let is_default = ExportFormat::ALL
            .iter()
            .any(|f| f.default_filename() == self.path_input.as_str());
        if is_default {
            self.path_input = self.selected_format().default_filename().to_string();
        }
    }
}
