# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`skelecode` is an interactive TUI-based code structure scanner that generates project-wide context graphs (Mermaid & Machine Context) for AI-assisted analysis.

- **Languages**: Rust, Java, Kotlin (WIP), JavaScript (WIP).
- **Architecture**: Tree-sitter based parsers → Universal IR → Multiple Renderers.
- **Interface**: Interactive TUI (ratatui) or CLI for piping.

## Commands

```bash
# Build (Standard)
cargo build

# Build for WSL (if encountering Windows file locks)
wsl -e bash -c "cargo build"

# Run (TUI Mode)
cargo run --bin skelecode

# Run (CLI Mode)
cargo run --bin skelecode -- <PATH> [OPTIONS]

# Test
cargo test

# Lint
cargo clippy

# Format
cargo fmt
```

## Tech Stack
- **Parsing**: `tree-sitter`, `tree-sitter-rust`, `tree-sitter-java`.
- **TUI**: `ratatui`, `crossterm`.
- **CLI**: `clap`.
