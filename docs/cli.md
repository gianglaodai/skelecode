# CLI Usage

## Synopsis

```
skelecode [OPTIONS] [PATH]
```

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<PATH>` | No | Path to the project directory. Omit to launch TUI with input screen. |

## Options

| Option | Short | Default | Description |
|---|---|---|---|
| `--format <FORMAT>` | `-f` | `vault` | Output format: `vault` (Obsidian), `machine`, or `both` |
| `--output <DIR/FILE>` | `-o` | stdout | Write output. For `vault`, expected to be a directory. |
| `--output-vault <DIR>` | | | Write Obsidian Vault to specific directory |
| `--output-machine <FILE>` | | | Write Machine Context output to specific file |
| `--lang <LANG>` | `-l` | auto-detect | Filter by language: `rust`, `javabased` (java/kotlin), `jsts` (js/ts), `python` |
| `--exclude <PATTERN>` | `-e` | | Glob patterns to exclude (can be specified multiple times) |
| `--tui` | | `false` | Launch interactive TUI mode (default when no output flags given) |
| `--verbose` | `-v` | `false` | Print progress information to stderr |
| `--help` | `-h` | | Print help information |
| `--version` | `-V` | | Print version |

## Examples

### Basic Usage

```bash
# Scan current directory in TUI mode
skelecode .

# Launch TUI with an interactive configuration screen (default)
skelecode
```

### Format Selection

```bash
# Obsidian Vault only (for human browsing)
skelecode ./my-project -f vault -o ./vault

# Machine Context only (for AI consumption)
skelecode ./my-project -f machine -o context.txt

# Both formats (default)
skelecode ./my-project -f both
```

### Output to Files

```bash
# Single format to file
skelecode ./my-project -f machine -o context.txt

# Both formats to separate files
skelecode ./my-project --output-vault docs/vault --output-machine docs/context.txt

# Vault to directory, Machine Context to stdout
skelecode ./my-project --output-vault docs/vault -f machine
```

### Language Filtering

```bash
# Only Java/Kotlin files
skelecode ./my-project -l javabased

# Java/Kotlin and JS/TS
skelecode ./my-project -l javabased -l jsts

# Only Rust
skelecode ./my-project -l rust
```

### Path Filtering

```bash
# Exclude test directories
skelecode ./my-project -e "**/test/**" -e "**/tests/**"

# Only scan specific packages
skelecode ./my-project -i "src/main/java/com/example/core/**"

# Exclude generated code
skelecode ./my-project -e "**/generated/**" -e "**/build/**"
```

### Performance & Exclusions

```bash
# Exclude build artifacts (though many are excluded by default)
skelecode ./my-project -e "**/target/**" -e "**/node_modules/**"

# Verbose output (scan stats to stderr)
skelecode ./my-project -v -f machine
```

### Combined

```bash
# Scan Java/Kotlin sources, exclude tests, output Machine Context to file
skelecode ./my-project -l javabased -e "**/test/**" -f machine -o ai-context.txt

# Generate documentation Vault
skelecode ./my-project -f vault --output-vault ./docs/architecture_vault

## TUI Mode Interaction

When running in interactive mode, the following keys are available:

| Key | Action |
|---|---|
| `↑` / `k` / `↓` / `j` | Navigate tree |
| `Enter` / `l` / `→` | Expand node / Toggle details |
| `h` / `←` | Collapse node / Jump to parent |
| `/` | **Symbol Search** (live filtering / highlighting) |
| `Tab` | Switch between **Machine Context** and **Obsidian Preview** |
| `y` | **Copy** current detail panel content to clipboard |
| `u` / `d` | **Scroll** detail panel up / down |
| `e` | Open Export Overlay |
| `g` / `G` | Jump to Top / Bottom |
| `b` / `Esc` | **Back** to Welcome Screen (configuration) |
| `q` | Quit |

### Export Overlay
Pressing `e` opens a non-blocking dialog where you can:
- Select export format (Vault / Machine).
- Cycle through options with `Tab` or `j`/`k`.
- Type an output path.
- Press `Enter` on the `[Export]` button to run.

## Output Behavior

- When `--output` / `--output-vault` / `--output-machine` is not specified, output goes to **stdout**
- When `--format both` with a single `--output`, Machine Context is written to the file and Vault is written to a subdirectory if output is a directory.
- Progress and diagnostic messages (with `--verbose`) go to **stderr**, keeping stdout clean for piping
- Exit code `0` on success, non-zero on error

## Language Auto-detection

When `--lang` is not specified, Skelecode detects languages by file extension:

| Extensions | Language |
|---|---|
| `.java` | Java |
| `.js`, `.jsx`, `.ts`, `.tsx` | JavaScript |
| `.kt`, `.kts` | Kotlin |
| `.rs` | Rust |
| `.py` | Python |

Files with unrecognized extensions are silently skipped.
