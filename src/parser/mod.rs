pub mod rust;
pub mod java;
pub mod kotlin;
pub mod jsts;
pub mod python;

use crate::ir::{Language, Module};
use std::path::Path;

pub trait LanguageParser {
    fn language(&self) -> Language;
    fn can_parse(&self, path: &Path) -> bool;
    fn parse_file(&self, path: &Path, source: &str) -> Option<Module>;
}

/// Detect language from file extension.
pub fn detect_language(path: &Path) -> Option<Language> {
    match path.extension()?.to_str()? {
        "rs" => Some(Language::Rust),
        "java" => Some(Language::Java),
        "js" | "jsx" | "ts" | "tsx" => Some(Language::JavaScript),
        "kt" | "kts" => Some(Language::Kotlin),
        "py" => Some(Language::Python),
        _ => None,
    }
}
