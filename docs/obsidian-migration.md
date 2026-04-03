# Obsidian Vault Migration: Re-architecting Diagram Generation

## The Breaking Point of Mermaid.js

Skelecode initially provided two output formats: **Machine Context** (for AI consumption) and **Mermaid.js** (for visual human consumption). While Mermaid.js `classDiagram` blocks work adequately for small microservices or isolated demonstrations, it drastically fails when tasked with mapping an established, enterprise-scale codebase.

When testing Skelecode against a ~68,000 LOC Java project, the following problems with the Mermaid renderer became evident:
1. **Rendering Engine Overload**: Trimming Mermaid diagrams by module only slightly mitigates the issue. When a core domains module contains dozens of classes with hundreds of interconnecting call-edges (`-->`) and inheritance arrows (`--|>`), the physics-based graph rendering engines inside VS Code plugins, GitHub, and Mermaid Live simply crash, time out, or produce unreadable overlapping spiderwebs.
2. **Loss of Interactivity**: A giant SVG/PNG representation of a codebase cannot be effectively searched, filtered, or intuitively browsed.
3. **Bloat**: Exporting static diagrams of a huge project wastes resources and negates the fundamental benefit of a "Structural Map".

## The Obsidian Vault Solution

To solve this, we are fundamentally shifting the paradigm of Skelecode's human-readable output:
**We are completely removing Mermaid.js and replacing it with an Obsidian Vault Generator.**

[Obsidian](https://obsidian.md/) is a wildly popular, local-first knowledge base application built entirely on Markdown files and WikiLinks (`[[Link]]`). By turning a source code repository into an Obsidian Vault, we map code architecture to a true Knowledge Graph.

### How it Translates
Instead of generating one giant static file (`diagram.md`), the new **Obsidian Renderer** exports a highly interconnected *directory* of Markdown files.

- **Vault Structure**: The Skelecode output becomes a folder (the "Vault") mimicking the module spaces.
- **Node = File**: Every `TypeDef` (Class, Interface, Struct, Record) becomes its own `.md` file. Within this file, Skelecode injects YAML frontmatter (metadata) and lists properties and methods.
- **Edge = WikiLink**: Every relationship (`extends`, `implements`, `@calls`) is represented as an Obsidian WikiLink (e.g., `[[UserRepository]]`). 
- **Graph View**: When the user opens this folder in Obsidian, Obsidian's native **Graph View** engine automatically plots every WikiLink into a scalable, interactive, 3D network. Users can dynamically filter scopes, color-code nodes based on language or module, and instantly click into a node to see its signature logic.

## Implementing the Migration (Phase 6)

1. **Delete Mermaid**: Remove `/renderer/mermaid.rs` entirely. Skelecode will no longer maintain the obsolete diagram syntaxes.
2. **Introduce Obsidian Renderer**: Create `/renderer/obsidian.rs`. This renderer will accept a Root Directory Path instead of a single File Path. 
3. **Change CLI**: Rename `--output-mermaid` to `--output-vault` (expecting a directory). Update descriptions.
4. **Update TUI**: Replace the "Mermaid" detail tab with an "Obsidian Preview" tab displaying the raw markdown shape of the node. Update the Export overlay logic to prompt for a directory path, executing a multi-file generator rather than a single string append. 

This pivot aligns perfectly with Skelecode's mission: to map massive architectures into ultra-scannable context networks—while leaving the heavy graph-visualization lifting to physics engines (like Obsidian) specifically designed for the job.
