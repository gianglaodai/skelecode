# Skelecode

**Code structure scanner that generates project-wide context graphs for humans and AI.**

Skelecode scans your source code and produces a compact structural representation, preserving types, methods, and call relationships while stripping implementation details. Perfect for giving LLMs project-wide context without hitting token limits.

## ✨ Features

- 🖥️ **Interactive TUI** — Browse modules, types, and methods in your terminal.
- 🦀 **Multi-language Support** — Built-in parsers for **Rust** and **Java** (Kotlin/JS planned).
- 🤖 **AI-Ready Output** — Export "Machine Context" format optimized for LLMs.
- 📊 **Visual Diagrams** — Export Mermaid class diagrams for documentation.
- 🚀 **Performance** — Fast, tree-sitter based analysis.

## 🚀 Quick Start

```bash
# Clone and build
git clone https://github.com/user/skelecode.git
cd skelecode
cargo build --release

# Launch TUI to scan a project
./target/release/skelecode /path/to/your/project
```

## 📖 Documentation

Detailed documentation is available in the [docs/](docs/README.md) folder:

- [**README**](docs/README.md) — Overview, installation, and TUI guide.
- [**CLI Usage**](docs/cli.md) — Command-line arguments and non-interactive mode.
- [**Architecture**](docs/architecture.md) — Internals, Unified IR, and Parser design.
- [**Output Formats**](docs/output-formats.md) — Mermaid and Machine Context specifications.

## 🛠️ Built With

- [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) — High-performance parsing.
- [Ratatui](https://ratatui.rs/) — Interactive terminal interface.
- [Clap](https://clap.rs/) — Robust CLI argument handling.

License: MIT
