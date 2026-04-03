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
| `--format <FORMAT>` | `-f` | `both` | Output format: `mermaid`, `machine`, or `both` |
| `--output <FILE>` | `-o` | stdout | Write output to file. When `--format both`, use this flag twice or use `--output-mermaid` / `--output-machine` |
| `--output-mermaid <FILE>` | | | Write Mermaid output to specific file |
| `--output-machine <FILE>` | | | Write Machine Context output to specific file |
| `--lang <LANG>` | `-l` | auto-detect | Filter by language: `java`, `js`, `kotlin`, `rust`. Can be specified multiple times |
| `--exclude <PATTERN>` | `-e` | | Glob patterns to exclude (e.g. `**/test/**`). Can be specified multiple times |
| `--include <PATTERN>` | `-i` | | Glob patterns to include. If specified, only matching files are scanned |
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
# Mermaid only (for documentation)
skelecode ./my-project -f mermaid

# Machine Context only (for AI consumption)
skelecode ./my-project -f machine

# Both formats (default)
skelecode ./my-project -f both
```

### Output to Files

```bash
# Single format to file
skelecode ./my-project -f machine -o context.txt

# Both formats to separate files
skelecode ./my-project --output-mermaid docs/diagram.md --output-machine docs/context.txt

# Mermaid to file, Machine Context to stdout
skelecode ./my-project --output-mermaid docs/diagram.md -f both
```

### Language Filtering

```bash
# Only Java files
skelecode ./my-project -l java

# Java and Kotlin
skelecode ./my-project -l java -l kotlin

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

### Performance Options

```bash
# Skip call graph analysis for faster structural overview
skelecode ./my-project --no-calls

# Verbose output (progress to stderr, results to stdout)
skelecode ./my-project -v -f machine -o context.txt
```

### Combined

```bash
# Scan Java/Kotlin sources, exclude tests, output Machine Context to file
skelecode ./my-project -l java -l kotlin -e "**/test/**" -f machine -o ai-context.txt

# Generate documentation diagrams
skelecode ./my-project -f mermaid --output-mermaid docs/architecture.md

### Interactive TUI mode

```bash
# Launch and browse a project interactively
skelecode ./my-project

# Launch TUI and pre-filter for Java
skelecode ./my-project -l java

# Launch TUI and exclude specific folders
skelecode ./my-project -e "**/target/**"
```

## Output Behavior

- When `--output` / `--output-mermaid` / `--output-machine` is not specified, output goes to **stdout**
- When `--format both` with a single `--output`, both formats are written to the same file separated by a header
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

Files with unrecognized extensions are silently skipped.
