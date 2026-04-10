# Project Roadmap & Evolution

This document tracks the phased evolution of Skelecode from inception to the stable v1.0 release.

## Completed Phases

### ✅ Phase 1: Core Foundation
- Defined Unified IR (Intermediate Representation).
- Implemented basic Rust parser with Tree-sitter.
- Created Machine Context renderer for AI-ready output.

### ✅ Phase 2: CLI & TUI
- Robust CLI with Clap.
- Interactive TUI using Ratatui.
- Support for language filtering and exclusions.

### ✅ Phase 3: Java Support
- Full Java parser (class, interface, enum, record).
- Integration with Unified IR.

### ✅ Phase 4: Kotlin Support
- Kotlin parser (data classes, objects, properties).
- Support for Kotlin-specific constructs.

### ✅ Phase 5: JavaScript & TypeScript Support
- Parsers for `.js`, `.ts`, `.jsx`, `.tsx`.
- Support for functional and class-based patterns.
- Module-as-file mapping for JS ecosystem.

### ✅ Phase 6: Obsidian Vault Migration
- Removed legacy Mermaid.js renderer due to scalability limits.
- Implemented **Obsidian Vault Renderer** generating interlinked Markdown files.
- Added **Visual Topology** via native Obsidian Canvas generation.

### ✅ Phase 7: Call Resolution (Level 1)
- Resolve `this`/`self` calls to current type.
- Resolve calls on fields with explicit types.
- Resolve calls on method/function parameters.
- `base_type()` strips `&mut`, `Box<>`, `Arc<>`, `Option<>`, `?` decorators.

### ✅ Phase 8: Call Resolution (Level 2)
- Parse `import`/`use` statements for all languages.
- Map type aliases to fully qualified paths.
- Cross-module call graph edges in Obsidian Graph View.
- Auto-exclude build output directories (`target/`, `bin/`, `build/`, etc.).

### ✅ Phase 9: UI Polish & Export Enhancements
- Richer Dataview/Juggl metadata in Obsidian vault (YAML frontmatter, inline fields, edge labels).
- Fields rendered as Markdown tables; relations with `edge-label::` hints.
- TUI "Obsidian Preview" tab shows per-type/method/function Obsidian markdown.
- Copy to Clipboard (`y`) for current detail panel content.
- Back navigation (`Esc`/`b`) from main view to welcome screen.
- Structure panel scrolling with `ListState`.
- Detail panel scrolling with `u`/`d`.

---

## Future Roadmap

### ✅ Phase 10: Reverse Call Graph
- Added `CallerRef` to IR; `callers: Vec<CallerRef>` on `Method` and `Function`.
- `resolve_reverse_calls()` post-processes forward call graph to populate reverse edges.
- Machine Context emits `@callers[...]` alongside `@calls[...]`.
- Obsidian vault emits `called-by::` inline fields with Juggl-compatible links.
- TUI Detail panel shows "Called by" section for method and function nodes.

### ✅ Phase 11: Python Support
- Parser for Python (classes, functions, imports, decorators).
- `typed_parameter` extraction, `__init__` field inference, `self.*` call tracking.
- Map to Unified IR; integrated into TUI, CLI, and Obsidian/Machine renderers.

### ✅ Phase 12: Symbol Search
- Press `/` to enter search mode; type to filter the structure tree in real-time.
- Matched text highlighted (yellow background) in tree; parent module shown as context.
- `Enter` commits filter (stays active for navigation); `Esc` clears filter.
- Match count shown in tree title bottom; search bar replaces help bar while active.
