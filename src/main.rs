use clap::{Parser, ValueEnum};
use skelecode::ir::Language;
use skelecode::renderer::Renderer;
use skelecode::renderer::machine::MachineRenderer;
use skelecode::renderer::mermaid::MermaidRenderer;
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Mermaid,
    Machine,
    Both,
}

#[derive(Debug, Clone, ValueEnum)]
enum LangFilter {
    Rust,
    Java,
    #[value(alias = "javascript", alias = "ts", alias = "typescript")]
    Js,
    Kotlin,
}

impl LangFilter {
    fn to_language(&self) -> Language {
        match self {
            LangFilter::Rust => Language::Rust,
            LangFilter::Java => Language::Java,
            LangFilter::Js => Language::JavaScript,
            LangFilter::Kotlin => Language::Kotlin,
        }
    }
}

/// Code structure scanner that generates project-wide context graphs for humans and AI.
#[derive(Parser, Debug)]
#[command(name = "skelecode", version, about)]
struct Cli {
    /// Path to the project directory to scan (optional — omit to use interactive TUI input screen)
    path: Option<PathBuf>,

    /// Output format (implies non-interactive mode)
    #[arg(short, long)]
    format: Option<OutputFormat>,

    /// Write output to file (stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Write Mermaid output to specific file
    #[arg(long)]
    output_mermaid: Option<PathBuf>,

    /// Write Machine Context output to specific file
    #[arg(long)]
    output_machine: Option<PathBuf>,

    /// Filter by language (can be specified multiple times)
    #[arg(short, long)]
    lang: Vec<LangFilter>,

    /// Glob patterns to exclude (can be specified multiple times)
    #[arg(short, long)]
    exclude: Vec<String>,

    /// Launch interactive TUI mode (default when no --format or --output is given)
    #[arg(long)]
    tui: bool,

    /// Print progress information to stderr
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();

    // Decide mode: TUI (interactive) vs CLI (pipe-friendly)
    let want_cli_output = cli.format.is_some()
        || cli.output.is_some()
        || cli.output_mermaid.is_some()
        || cli.output_machine.is_some();

    let use_tui = cli.tui || !want_cli_output;

    // ── TUI mode ───────────────────────────────────────────────────────────────
    if use_tui {
        match &cli.path {
            None => {
                // No path given → open the welcome/input screen
                if let Err(e) = skelecode::tui::run_tui_welcome() {
                    eprintln!("TUI error: {}", e);
                    std::process::exit(1);
                }
            }
            Some(path) => {
                // Path given → validate, scan, open main view directly
                validate_path(path);

                if cli.verbose {
                    eprintln!("Scanning {}...", path.display());
                }

                let languages: Vec<Language> =
                    cli.lang.iter().map(|l| l.to_language()).collect();
                let project = skelecode::scan_project(path, &languages, &cli.exclude);

                if cli.verbose {
                    print_scan_stats(&project);
                }

                if let Err(e) = skelecode::tui::run_tui(project) {
                    eprintln!("TUI error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        return;
    }

    // ── CLI mode ────────────────────────────────────────────────────────────────
    let path = cli.path.as_ref().unwrap_or_else(|| {
        eprintln!("Error: <PATH> is required in non-interactive (--format) mode.");
        eprintln!("Usage: skelecode <PATH> [OPTIONS]");
        std::process::exit(1);
    });

    validate_path(path);

    if cli.verbose {
        eprintln!("Scanning {}...", path.display());
    }

    let languages: Vec<Language> = cli.lang.iter().map(|l| l.to_language()).collect();
    let project = skelecode::scan_project(path, &languages, &cli.exclude);

    if cli.verbose {
        print_scan_stats(&project);
    }

    let format = cli.format.unwrap_or(OutputFormat::Both);

    let mermaid_output = match format {
        OutputFormat::Mermaid | OutputFormat::Both => Some(MermaidRenderer.render(&project)),
        _ => None,
    };

    let machine_output = match format {
        OutputFormat::Machine | OutputFormat::Both => Some(MachineRenderer.render(&project)),
        _ => None,
    };

    // Write outputs
    if let Some(ref path) = cli.output_mermaid
        && let Some(ref content) = mermaid_output
    {
        write_file(path, content);
    }

    if let Some(ref path) = cli.output_machine
        && let Some(ref content) = machine_output
    {
        write_file(path, content);
    }

    // If specific output files are set, don't write to --output or stdout for those formats
    let mermaid_to_general = mermaid_output
        .as_ref()
        .filter(|_| cli.output_mermaid.is_none());
    let machine_to_general = machine_output
        .as_ref()
        .filter(|_| cli.output_machine.is_none());

    let mut general_output = String::new();

    if let Some(content) = mermaid_to_general {
        if !general_output.is_empty() {
            general_output.push_str("\n---\n\n");
        }
        general_output.push_str("# Mermaid Diagram\n\n");
        general_output.push_str(content);
    }

    if let Some(content) = machine_to_general {
        if !general_output.is_empty() {
            general_output.push_str("\n---\n\n");
        }
        general_output.push_str("# Machine Context\n\n");
        general_output.push_str(content);
    }

    if !general_output.is_empty() {
        if let Some(ref path) = cli.output {
            write_file(path, &general_output);
        } else {
            print!("{}", general_output);
        }
    }
}

fn validate_path(path: &PathBuf) {
    if !path.exists() {
        eprintln!("Error: path '{}' does not exist", path.display());
        std::process::exit(1);
    }
    if !path.is_dir() {
        eprintln!("Error: '{}' is not a directory", path.display());
        std::process::exit(1);
    }
}

fn print_scan_stats(project: &skelecode::ir::Project) {
    let type_count: usize = project.modules.iter().map(|m| m.types.len()).sum();
    let fn_count: usize = project.modules.iter().map(|m| m.functions.len()).sum();
    eprintln!(
        "Found {} modules, {} types, {} free functions",
        project.modules.len(),
        type_count,
        fn_count,
    );
}

fn write_file(path: &PathBuf, content: &str) {
    if let Err(e) = std::fs::write(path, content) {
        eprintln!("Error writing to {}: {}", path.display(), e);
        std::process::exit(1);
    }
}
