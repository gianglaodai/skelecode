# Skelecode

**Code structure scanner that generates project-wide context graphs for humans and AI.**

## Problem

When AI assistants work with codebases, they face a dilemma:
- **Scan everything** → expensive token consumption, slow
- **Scan partially** → missing context, duplicate code, unused common functions

Skelecode solves this by scanning the project once and producing a **compact structural graph** that captures classes, fields, methods, call relationships, and type hierarchies — without including implementation details.

## How It Works

```
Source Code  ──►  Parser  ──►  Resolution Layer  ──►  Unified IR  ──►  Output
  .java           (per lang)      (Level 1 & 2)       (agnostic)      ├── TUI (interactive)
  .js/.ts                                                             ├── Obsidian (human)
  .kt                                                                 └── Machine Context (AI)
  .rs
```

1. **Parse** — language-specific parsers extract structural information from source files
2. **Model** — parsed data is normalized into a unified intermediate representation (IR)
3. **Render** — the IR is presented via interactive TUI, or rendered to Obsidian Vault / Machine Context formats

## Output Modes

| Mode | Audience | Description |
|---|---|---|
| **TUI** | Developers | Interactive terminal UI — browse modules, types, methods with keyboard navigation |
| **Obsidian** | Humans | Interactive knowledge graph and visual canvas for browsing code architecture |
| **Machine Context** | AI | Ultra-compact token-optimized format for LLM consumption |

Estimated token savings with Machine Context: **~90-95% reduction** compared to reading raw source code, while preserving structural and relational information.

## Supported Languages

| Language | Extensions | Key Constructs | Status |
|---|---|---|---|
| Rust | `.rs` | struct, enum, trait, impl, mod | Stable |
| Java | `.java` | class, interface, enum, record, annotations | Stable |
| JavaScript | `.js`, `.ts`, `.jsx`, `.tsx` | class, function, arrow function, export | Stable |
| Kotlin | `.kt`, `.kts` | class, interface, object, data class, annotations | Stable |
| Python | `.py` | class, function, annotations, import | Stable |

## Installation

### Build from source

```bash
# Clone the repository
git clone https://github.com/user/skelecode.git
cd skelecode

# Build release binary
cargo build --release

# The binary is at target/release/skelecode
```

Requirements:
- Rust 1.87+ (edition 2024)
- C compiler (for tree-sitter) — `gcc` on Linux, MSVC Build Tools on Windows

### Windows note

On Windows with Git Bash, the GNU `link.exe` may shadow the MSVC linker. Either:
- Build from **Developer Command Prompt for Visual Studio**, or
- Build in WSL: `wsl -e bash -c "source ~/.cargo/env && cargo build --release"`

## Quick Start

### Interactive mode (TUI) — default

```bash
# Scan a project and browse interactively
skelecode /path/to/project

# Scan with language filter
skelecode /path/to/project --lang rust

# Explicit TUI flag
skelecode /path/to/project --tui
```

The TUI launches automatically when no `--format` or `--output` flags are given.

### TUI keybindings

```
  ↑ / k         Move up
  ↓ / j         Move down
  → / l / Enter  Expand node / Toggle details
  ← / h         Collapse node / jump to parent
  /             Symbol Search (live filter)
  Tab           Switch detail panel (Machine Context ↔ Obsidian Preview)
  u / d         Scroll detail panel up / down
  y             Copy current detail panel to clipboard
  g / G         Jump to top / bottom
  e             Open Export Overlay (Machine Context / Obsidian Vault)
  b / Esc       Back to Welcome Screen
  q             Quit
```

### TUI layout

```
┌─ Structure ──────────────────┬─ Detail [Machine Context] ──────────┐
│ 📦 @mod parser [rust]        │ @type Parser [struct]                │
│   ▶ Parser [struct]          │   {source:String, pos:usize}        │
│   ▶ Lexer [struct]           │   @vis pub                          │
│     fn new(String)->Self     │   @impl Display                     │
│     fn parse()->Result       │                                     │
│   ƒ helper()                 │ Fields:                             │
│                              │   pub source : String                │
│                              │   private pos : usize                │
│                              │                                     │
│                              │ Methods (3):                        │
│                              │   new(String)->Self [static]        │
│ 3 modules, 12 types          │   parse()->Result<AST>              │
├──────────────────────────────┴─────────────────────────────────────┤
│ ↑↓ Navigate  ←→ Expand  / Search  Tab [Machine]  y Copy  e Export  │
│ b Back  q Quit                                                     │
└────────────────────────────────────────────────────────────────────┘
```

- **Left panel** — tree view: modules → types → methods/functions. Expand/collapse with arrow keys.
- **Right panel** — detail view for the selected item: fields, parameters, call graph, relations.
- **Bottom bar** — keybinding reference and active detail tab.

### CLI mode (non-interactive)

Use `--format` or `--output` to switch to CLI mode, suitable for piping and file generation.

```bash
# Output Machine Context to stdout
skelecode /path/to/project --format machine

# Output Obsidian Vault to directory
skelecode /path/to/project --format vault -o ./vault

# Output Machine Context to file
skelecode /path/to/project --format machine -o context.txt

# Both formats to separate locations
skelecode /path/to/project --output-vault ./vault --output-machine context.txt

# Filter by language, exclude test directories
skelecode /path/to/project --lang rust --exclude "**/test/**"

# Verbose mode (progress to stderr)
skelecode /path/to/project --format machine -v
```

See [CLI Usage](cli.md) for the full list of options.

## Example Output

### Machine Context

```
@lang rust
@mod parser
@type Parser [struct] {source:String, pos:usize}
  @vis pub
  @fn new(String)->Self @vis pub @static
  @fn parse()->Result<AST> @vis pub @calls[Lexer::tokenize, AST::new]
  @impl Display

@type Lexer [struct] {input:String}
  @vis pub
  @fn tokenize(&str)->Vec<Token> @vis pub
```

### Obsidian Topology (Canvas)

Skelecode generates a `Topology.canvas` file that provides a visual map of the project architecture with labeled relationship arrows, viewable natively in Obsidian.

- **Modules** are containers.
- **Types** are nodes.
- **Arrows** represent inheritance and calls.

## Documentation

- [Architecture](architecture.md) — system design, parser layer, unified IR, renderer layer
- [Output Formats](output-formats.md) — detailed specification of Obsidian Vault and Machine Context formats
- [CLI Usage](cli.md) — command-line arguments and options
- [Call Resolution](call-resolution.md) — strategy and roadmap for resolving method call targets across languages
- [Implementation Plan](plan.md) — phased roadmap with task tracking
