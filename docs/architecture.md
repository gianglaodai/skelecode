# Architecture

## Overview

Skelecode follows a three-layer pipeline architecture:

```
│ Parser Layer │    │ Resolution Layer │    │  Unified IR  │──┬─►│Renderer Layer│
│ (per lang)   │───►│ (Phase 7, 8, 10) │───►│  (language-  │  │  │ - Obsidian   │
│              │    │                  │    │   agnostic)  │  │  │ - Machine    │
│  tree-sitter │    │   heuristics     │    └──────────────┘  │  └──────────────┘
└─────────────┘    └──────────────────┘                      │
                                                              ▼
                                                        ┌────────────┐
                                                        │ TUI Layer  │
                                                        │ (ratatui)  │
                                                        └────────────┘
```

Each layer has a single responsibility and communicates through well-defined data structures.

## Parser Layer

### Purpose

Extract structural information from source code files. Each language has its own parser, but all parsers produce the same unified IR.

### Technology: tree-sitter

All parsers are built on [tree-sitter](https://tree-sitter.github.io/tree-sitter/) — an incremental parsing library that provides concrete syntax trees for source files.

Why tree-sitter:
- **Multi-language** — grammar available for Java, JS/TS, Kotlin, Rust, and more
- **Robust** — handles partial/invalid syntax gracefully
- **Fast** — incremental parsing
- **Rust bindings** — first-class `tree-sitter` crate support

### Per-language Parsers

Each parser is responsible for:

1. **Identifying type containers** — classes, structs, interfaces, enums, traits, objects
2. **Extracting fields/properties** — name, type, visibility
3. **Extracting methods/functions** — name, parameters, return type, visibility
4. **Resolving call relationships** — which methods/functions are called within each method body
5. **Resolving type relationships** — inheritance, interface implementation, trait implementation
6. **Extracting metadata** — annotations (Java/Kotlin), decorators (JS/TS), attributes (Rust)

### Language-specific Mapping

```
Java:
  class/interface/enum/record  → TypeDef
  field                        → Field
  method/constructor           → Method
  extends/implements           → TypeRelation
  @annotation                  → Annotation

JavaScript/TypeScript:
  class                        → TypeDef
  function/arrow function      → Function (module-level) or Method (in class)
  export                       → Visibility::Public
  property                     → Field
  extends                      → TypeRelation

Kotlin:
  class/interface/object/
    data class/sealed class    → TypeDef
  property (val/var)           → Field
  fun                          → Method
  : SuperClass/Interface       → TypeRelation
  @annotation                  → Annotation

Rust:
  struct/enum                  → TypeDef
  trait                        → TypeDef (trait)
  impl Trait for Type          → TypeRelation
  impl Type { fn ... }         → Method
  fn (module-level)            → Function
  pub/pub(crate)               → Visibility

Python:
  class                        → TypeDef
  def (in class)               → Method
  def (at module level)        → Function
  bases (parent classes)       → TypeRelation
  @decorator                   → Annotation
  self.x = ... (in __init__)   → Field

## Resolution Layer

### Purpose

Raw parse results often contain ambiguous references (e.g., `self.repo`, `this.service`). The Resolution Layer resolves these into concrete type names across the entire project.

### Phase 1: Local Resolution (Level 1)
Determines the type of a receiver based on the local scope (class/method).
- **Self/This**: Maps to the current type name.
- **Fields**: Maps `self.field` to the declared type of the field.
- **Parameters**: Resolves calls on method arguments by checking their declared type.
- **Type Stripping**: `base_type()` recursively strips decorators like `&mut`, `Box<>`, `Arc<>`, `Option<>`, and Kotlin's `?`.

### Phase 2: Global Resolution (Level 2)
Handles cross-module references using import/use statements.
- **Alias Mapping**: Builds a map of imported types (e.g., `import com.pkg.User as U`).
- **Qualified Names**: Replaces imported aliases with fully qualified paths (e.g., `U` → `com.pkg::User`).
- **Graph Consistency**: Ensures that even if two classes have the same name in different modules, the call graph correctly links to the intended destination.

### Phase 3: Reverse Call Graph (Phase 10)
After all calls are resolved, a final pass iterates through the graph and populates "called-by" information.
- **Bi-directional Mapping**: Enables deep structural analysis by answering "who uses this method?".
- **Reflective Search**: Populates `callers` vectors in `Method` and `Function` structs.
```

## Unified IR (Intermediate Representation)

### Purpose

A language-agnostic data model that captures the structural essence of any supported codebase. This is the single source of truth between parsing and rendering.

### Core Model

```
Project
├── modules: Vec<Module>
│
Module
├── path: String              # e.g. "com.example.service" or "src/parser"
├── language: Language         # Java | JavaScript | Kotlin | Rust | Python
├── types: Vec<TypeDef>
├── functions: Vec<Function>  # module-level functions (not in a type)
│
TypeDef
├── name: String
├── kind: TypeKind            # Class | Interface | Enum | Struct | Trait | Object | Record
├── visibility: Visibility    # Public | Private | Protected | Internal | Crate
├── fields: Vec<Field>
├── methods: Vec<Method>
├── relations: Vec<TypeRelation>
├── annotations: Vec<Annotation>
├── type_params: Vec<String>  # generics: <T, U>
├── enum_variants: Vec<String>
│
Field
├── name: String
├── type_name: String
├── visibility: Visibility
│
Method
├── name: String
├── params: Vec<Param>        # (name, type)
├── return_type: Option<String>
├── visibility: Visibility
├── calls: Vec<CallRef>       # direct calls to other methods/functions
├── callers: Vec<CallerRef>   # reverse calls: who calls this?
├── annotations: Vec<Annotation>
├── is_static: bool
│
Function                       # module-level (not belonging to a type)
├── name: String
├── params: Vec<Param>
├── return_type: Option<String>
├── visibility: Visibility
├── calls: Vec<CallRef>
├── callers: Vec<CallerRef>
│
CallRef
├── target_type: Option<String>  # None for free functions
├── target_method: String
│
TypeRelation
├── kind: RelationKind        # Extends | Implements | ImplTrait
├── target: String            # target type name
│
Annotation
├── name: String

CallerRef
├── source_type: Option<String> # None for free functions
├── source_method: String
```

### Design Decisions

1. **No implementation bodies** — only signatures and call references are stored.
2. **Direct calls only** — each method lists what it directly calls; transitive paths are derived from the graph.
3. **String-based type references** — types reference each other by name (not pointers), keeping the model serializable and simple.
4. **Annotations as flat names** — annotation arguments are not captured (reduces complexity, rarely needed for structural understanding).

## Design Decisions: The Shift to Obsidian

Initially, Skelecode used **Mermaid.js** for visual output. This was removed in Phase 6 due to several "breaking points" encountered in large-scale projects:

- **Rendering Overload**: For codebases >50k LOC, static SVG/PNG renderers (like Mermaid) crash or produce "spiderwebs" that are impossible to read.
- **Loss of Interactivity**: Architecture is best explored through a zoomable, clickable knowledge graph, not a static image.

### The Obsidian Paradigm
By treating the codebase as a **Knowledge Graph** (Notes = Types, Links = Relationships), we leverage Obsidian's high-performance Graph View and Canvas engines to provide a truly scalable architectural map.

## Renderer Layer

### Purpose

Transform the unified IR into human-readable or machine-readable output.

### Obsidian Vault Renderer

Produces an interactive Obsidian Vault (directory of Markdown files) with a visual `Topology.canvas` map. Details in [output-formats.md](output-formats.md#obsidian-vault-format).

### Machine Context Renderer

Produces the compact `@`-tag based format. Details in [output-formats.md](output-formats.md#machine-context-format).

## TUI Layer (Interactive)

### Purpose
Provides a terminal-base UI for real-time project browsing and on-the-fly export configuration.

### Components
- **WelcomeApp** (`src/tui/welcome_app.rs`): Handles initial path input, language filtering, and exclude patterns.
- **Main App** (`src/tui/app.rs`): Displays a hierarchical tree of the scanned project.
- **Export Overlay** (`src/tui/export.rs`): A non-blocking overlay to configure and execute exports (Vault/Machine) to directories/files.
- **UI & Themes** (`src/tui/ui.rs`): Handles layout, highlighters, and visual themes using `ratatui`.

## Directory Structure

```
src/
├── main.rs                  # CLI entry point & routing
├── lib.rs                   # Core logic & parser registry
├── ir/                      # Unified IR model
│   └── mod.rs               # Project, Module, TypeDef, etc.
├── parser/                  # Parser layer
│   ├── mod.rs               # Parser trait + language detection
│   ├── java.rs              # Java parser
│   ├── jsts.rs              # Unified JavaScript/TypeScript parser
│   ├── kotlin.rs            # Kotlin parser
│   ├── python.rs            # Python parser
│   └── rust.rs              # Rust parser
├── renderer/                # Renderer layer
│   ├── mod.rs               # Renderer trait
│   ├── canvas.rs            # Obsidian Canvas generation
│   ├── machine.rs           # Compact context generation
│   └── obsidian.rs          # Vault structure generation
├── resolver/                # Resolution layer
│   └── mod.rs               # Call & Import resolution logic
└── tui/                     # TUI Layer
    ├── mod.rs               # Runner & main loop
    ├── app.rs               # Main navigation app logic
    ├── export.rs            # Export overlay logic
    ├── ui.rs                # View drawing & styles
    └── welcome.rs           # Configuration screen
```
