# Changelog

All notable changes to this project will be documented in this file.

## [1.0.0] - 2026-04-10

### Added
- **Multi-language Support (Phase 1-5, 11)**:
    - Stable parsers for **Rust, Java, Kotlin, JavaScript, TypeScript, and Python**.
    - Language-specific mapping to Unified IR.
    - `__init__` field inference and static decorator support for Python.
- **Advanced Call Resolution (Phase 7-8)**:
    - **Level 1**: Local heuristic resolution (`self`, `this`, fields, params).
    - **Level 2**: Import-aware resolution mapping aliases to full module paths.
- **Reverse Call Graph (Phase 10)**:
    - Bi-directional graph linking targets back to their callers (`called-by::`, `@callers`).
- **Interactive TUI (Phase 2, 9, 12)**:
    - Real-time **Symbol Search** (`/`) with highlighting and context-aware filtering.
    - Interactive "Obsidian Preview" tab.
    - Export overlay for Vault/Machine formats.
    - Clipboard support (`y`) and advanced scrolling/navigation.
- **Obsidian Integration (Phase 6, 9)**:
    - Automated **Obsidian Vault** generation with Wikilinks.
    - **Visual Topology** via native Obsidian Canvas integration.
    - Rich metadata using Dataview and Juggl inline fields.
- **AI-Optimized Output**:
    - **Machine Context** format with 90-95% token reduction compared to raw source.

### Changed
- Standardized file paths for binary exports on Windows.
- Optimized graph walker for Phase 10 reverse edge population.
- Improved terminal UI responsiveness.

### Fixed
- Fixed Windows path separator issues in generated Wikilinks.
- Resolved ambiguous receiver types in Kotlin scope functions.

---
*v1.0.0 represents the first stable release of Skelecode.*
