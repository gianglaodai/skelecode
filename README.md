# Skelecode

**Code structure scanner that generates project-wide context graphs for humans and AI.**

> [!NOTE]
> **Stable Release v1.0.0** — All core phases (1-12) are completed.

Skelecode scans your source code and produces a compact structural representation, preserving types, methods, and call relationships while stripping implementation details. Perfect for giving LLMs project-wide context without hitting token limits.

## ✨ Features

- 🖥️ **Interactive TUI** — Browse modules, types, and methods with live **Obsidian Preview**.
- 🦀 **Multi-language Support** — Built-in parsers for **Rust**, **Java**, **Kotlin**, **JavaScript**, **TypeScript**, and **Python**.
- 🕵️ **Symbol Search** — Real-time TUI jump-to-symbol with live filtering and highlighting.
- 🕸️ **Advanced Call Graph** — Heuristic call resolution (Level 1 & 2) and **Reverse Call Graphs** (Phase 10).
- 🤖 **AI-Ready Output** — Export "Machine Context" format optimized for LLMs.
- 📂 **Obsidian Vault** — Generate an interactive knowledge graph with rich Dataview metadata.
- 🚀 **Performance** — Fast, tree-sitter based analysis with builtin exclusion for build artifacts.

## 🚀 Quick Start

```bash
# Clone and build
git clone https://github.com/gianglaodai/skelecode.git
cd skelecode
cargo build --release

# Launch TUI to scan a project
./target/release/skelecode /path/to/your/project
```

## ⌨️ TUI Keybindings

| Key | Action |
|---|---|
| `Enter` / `l` / `→` | Expand node / Toggle details |
| `h` / `←` | Collapse node / Jump to parent |
| `/` | **Symbol Search** (live filter) |
| `Tab` | Switch between Machine Context & Obsidian Preview |
| `y` | **Copy** current detail panel to clipboard |
| `u` / `d` | **Scroll** detail panel up/down |
| `e` | Open Export Overlay |
| `b` / `Esc` | Back to Welcome Screen |
| `q` | Quit |

## 📖 Documentation

Detailed documentation is available in the [docs/](docs/README.md) folder:

- [**README**](docs/README.md) — Overview, installation, and TUI guide.
- [**CLI Usage**](docs/cli.md) — Command-line arguments and non-interactive mode.
- [**Architecture**](docs/architecture.md) — Internals, Unified IR, and Parser design.
- [**Output Formats**](docs/output-formats.md) — Obsidian Vault and Machine Context specifications.

## 🛠️ Built With

- [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) — High-performance parsing.
- [Ratatui](https://ratatui.rs/) — Interactive terminal interface.
- [Clap](https://clap.rs/) — Robust CLI argument handling.

License: MIT
