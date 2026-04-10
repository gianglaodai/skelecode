pub mod ir;
pub mod parser;
pub mod renderer;
pub mod resolver;
pub mod tui;

use std::path::Path;
use walkdir::WalkDir;

use ir::{Language, Project};
use parser::LanguageParser;
use parser::rust::RustParser;
use parser::java::JavaParser;
use parser::kotlin::KotlinParser;
use parser::jsts::JsTsParser;
use parser::python::PythonParser;

/// Scan a project directory and produce a Project IR.
pub fn scan_project(root: &Path, languages: &[Language], exclude_patterns: &[String]) -> Project {
    let parsers: Vec<Box<dyn LanguageParser>> = create_parsers(languages);
    let mut modules = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_hidden(e) && !is_excluded(e, exclude_patterns))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        for parser in &parsers {
            if parser.can_parse(path) {
                if let Ok(source) = std::fs::read_to_string(path)
                    && let Some(module) = parser.parse_file(path, &source)
                {
                    modules.push(module);
                }
                break;
            }
        }
    }

    let mut project = Project { modules };
    resolver::resolve_calls(&mut project);
    resolver::resolve_import_calls(&mut project);
    resolver::resolve_reverse_calls(&mut project);
    project
}

fn create_parsers(languages: &[Language]) -> Vec<Box<dyn LanguageParser>> {
    let all_parsers: Vec<Box<dyn LanguageParser>> = vec![
        Box::new(RustParser::new()),
        Box::new(JavaParser::new()),
        Box::new(KotlinParser::new()),
        Box::new(JsTsParser::new()),
        Box::new(PythonParser::new()),
    ];

    if languages.is_empty() {
        return all_parsers;
    }

    all_parsers
        .into_iter()
        .filter(|p| languages.contains(&p.language()))
        .collect()
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| s.starts_with('.'))
}

/// Directory names that are always excluded regardless of user patterns.
/// These are build output / dependency / cache directories that commonly
/// contain copies of source files, causing duplicate parse results.
const BUILTIN_EXCLUDE_DIRS: &[&str] = &[
    // JVM build outputs (Gradle, Maven, IntelliJ)
    "bin", "build", "out", "target",
    // JS/TS package directories
    "node_modules", ".next", "dist",
    // Python
    "__pycache__", ".venv", "venv",
    // General caches / IDE metadata
    ".gradle", ".idea", ".cache",
];

fn is_excluded(entry: &walkdir::DirEntry, patterns: &[String]) -> bool {
    // Only apply builtin exclusions to directories (not individual files)
    if entry.file_type().is_dir() {
        if let Some(name) = entry.file_name().to_str() {
            if BUILTIN_EXCLUDE_DIRS.contains(&name) {
                return true;
            }
        }
    }

    let path_str = entry.path().to_string_lossy();
    for pattern in patterns {
        // Simple glob matching: check if path contains the pattern (without **)
        let clean = pattern.replace("**", "").replace('*', "");
        if !clean.is_empty() && path_str.contains(&clean) {
            return true;
        }
    }
    false
}
