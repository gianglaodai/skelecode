use clap::{Parser, ValueEnum};
use skelecode::ir::Language;
use skelecode::renderer::Renderer;
use skelecode::renderer::machine::MachineRenderer;
use skelecode::renderer::obsidian::ObsidianRenderer;
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Vault,
    Machine,
    Both,
}

#[derive(Debug, Clone, ValueEnum)]
enum LangFilter {
    Rust,
    #[value(alias = "java", alias = "kotlin", alias = "kt")]
    JavaBased,
    #[value(alias = "javascript", alias = "typescript", alias = "ts")]
    JsTs,
    #[value(alias = "python", alias = "py")]
    Python,
}

impl LangFilter {
    fn to_languages(&self) -> Vec<Language> {
        match self {
            LangFilter::Rust => vec![Language::Rust],
            LangFilter::JavaBased => vec![Language::Java, Language::Kotlin],
            LangFilter::JsTs => vec![Language::JavaScript],
            LangFilter::Python => vec![Language::Python],
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

    /// Write Vault output to specific directory
    #[arg(long)]
    output_vault: Option<PathBuf>,

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
        || cli.output_vault.is_some()
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
                    cli.lang.iter().flat_map(|l| l.to_languages()).collect();
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

    let languages: Vec<Language> = cli.lang.iter().flat_map(|l| l.to_languages()).collect();
    let project = skelecode::scan_project(path, &languages, &cli.exclude);

    if cli.verbose {
        print_scan_stats(&project);
    }

    let format = cli.format.unwrap_or(OutputFormat::Both);

    let vault_output = match format {
        OutputFormat::Vault | OutputFormat::Both => Some(ObsidianRenderer.render(&project)),
        _ => None,
    };

    let machine_output = match format {
        OutputFormat::Machine | OutputFormat::Both => Some(MachineRenderer.render(&project)),
        _ => None,
    };

    // Write outputs
    if let Some(ref path) = cli.output_vault
        && let Some(skelecode::renderer::RenderOutput::Multiple(files)) = vault_output.as_ref()
    {
        write_vault(path, files);
    }

    if let Some(ref path) = cli.output_machine
        && let Some(skelecode::renderer::RenderOutput::Single(content)) = machine_output.as_ref()
    {
        write_file(path, content);
    }

    // If --output is set, write the general outputs
    // Vault cannot be written to a single file, so if OutputFormat::Vault is selected
    // and neither output_vault nor output is set, we use stdout just for Machine or error?
    // Let's decide: if they use --output with Vault, they mean a directory.
    let mut general_machine = None;

    if let Some(skelecode::renderer::RenderOutput::Single(content)) = machine_output {
        if cli.output_machine.is_none() {
            general_machine = Some(content);
        }
    }

    if let Some(skelecode::renderer::RenderOutput::Multiple(files)) = vault_output {
        if cli.output_vault.is_none() {
            if let Some(ref path) = cli.output {
                write_vault(path, &files);
            } else {
                eprintln!("Warning: Cannot write Vault format to stdout. Use --output or --output-vault.");
            }
        }
    }

    if let Some(content) = general_machine {
        if let Some(ref path) = cli.output {
            // Note: If both vault and machine are writing to --output, it's ambiguous.
            // But if --output is a dir for vault, we might write machine to `output/machine.md`?
            // Let's just write to path. If they mix, they should use specific flags.
            if path.is_dir() {
                write_file(&path.join("MachineContext.md"), &content);
            } else {
                write_file(path, &content);
            }
        } else {
            println!("{}", content);
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

fn write_vault(base_path: &PathBuf, files: &[(PathBuf, String)]) {
    if let Err(e) = std::fs::create_dir_all(base_path) {
        eprintln!("Error creating directory {}: {}", base_path.display(), e);
        std::process::exit(1);
    }
    
    // Create subdirectories
    let modules_dir = base_path.join("modules");
    let types_dir = base_path.join("types");
    
    let _ = std::fs::create_dir_all(&modules_dir);
    let _ = std::fs::create_dir_all(&types_dir);

    for (rel_path, content) in files {
        let full_path = base_path.join(rel_path);
        if let Err(e) = std::fs::write(&full_path, content) {
            eprintln!("Error writing vault file {}: {}", full_path.display(), e);
        }
    }
}
