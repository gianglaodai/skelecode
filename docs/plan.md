# Implementation Plan

## Phase 1 — Foundation + Rust Parser (MVP)

Goal: end-to-end pipeline working with one language.

### 1.1 Project Setup
- [ ] Add dependencies to `Cargo.toml`: `clap`, `tree-sitter`, `tree-sitter-rust`
- [ ] Set up module structure: `ir/`, `parser/`, `renderer/`

### 1.2 Unified IR
- [ ] Define core types: `Project`, `Module`, `TypeDef`, `Field`, `Method`, `Function`, `CallRef`, `TypeRelation`, `Annotation`
- [ ] Define enums: `Language`, `TypeKind`, `Visibility`, `RelationKind`

### 1.3 Rust Parser
- [ ] File discovery: walk directory, filter `.rs` files
- [ ] Parse with tree-sitter-rust: extract `struct`, `enum`, `trait`
- [ ] Extract fields from struct definitions
- [ ] Extract `fn` from `impl` blocks → `Method`, module-level `fn` → `Function`
- [ ] Extract visibility (`pub`, `pub(crate)`, private)
- [ ] Extract `impl Trait for Type` → `TypeRelation`
- [ ] Call resolution Level 0: raw syntactic calls from method bodies

### 1.4 Machine Context Renderer
- [ ] Implement `@lang`, `@mod`, `@type`, `@fn`, `@field` tags
- [ ] Implement `@vis`, `@impl`, `@ext`, `@calls`, `@static`, `@enum` tags
- [ ] Output to stdout or file

### 1.5 Mermaid Renderer
- [ ] Generate `classDiagram` blocks per module
- [ ] Map visibility to Mermaid markers (`+`, `-`, `#`, `~`)
- [ ] Generate relationship arrows
- [ ] Generate type kind stereotypes (`<<interface>>`, `<<enum>>`, etc.)

### 1.6 CLI (basic)
- [ ] `<PATH>` argument
- [ ] `--format` flag: `mermaid`, `machine`, `both`
- [ ] `--output` flag
- [ ] `--lang` flag (only `rust` available in this phase)

### 1.7 Testing
- [ ] Create test fixtures: sample Rust files with known structures
- [ ] Unit tests for Rust parser
- [ ] Unit tests for both renderers
- [ ] Integration test: Rust file → parse → render → verify output

**Deliverable**: `skelecode ./rust-project -f machine` works end-to-end.

---

## Phase 2 — Java Parser

### 2.1 Java Parser
- [ ] Add `tree-sitter-java` dependency
- [ ] Parse: `class`, `interface`, `enum`, `record`
- [ ] Extract fields, methods, constructors
- [ ] Extract visibility (`public`, `private`, `protected`, package-private)
- [ ] Extract `extends`, `implements` → `TypeRelation`
- [ ] Extract annotations (`@Override`, `@Service`, etc.)
- [ ] Call resolution Level 0: raw syntactic calls

### 2.2 Call Resolution — Level 1 (Phase 1 of resolution plan)
- [ ] Resolve `this`/`self` calls for Rust and Java
- [ ] Resolve static/type-level calls (`ClassName.method()`, `Type::method()`)
- [ ] Resolve calls on fields with known types (field declarations in same class)
- [ ] Resolve calls on parameters with explicit types

### 2.3 Testing
- [ ] Test fixtures: sample Java files
- [ ] Unit tests for Java parser
- [ ] Unit tests for Level 1 call resolution
- [ ] Integration test: Java project end-to-end

**Deliverable**: Java + Rust support, calls partially resolved.

---

## Phase 3 — JavaScript/TypeScript Parser

### 3.1 JS/TS Parser
- [ ] Add `tree-sitter-javascript` and `tree-sitter-typescript` dependencies
- [ ] Parse: `class`, `function`, arrow functions
- [ ] Extract `export` → `Visibility::Public`
- [ ] Extract class fields/properties, methods
- [ ] Extract `extends` → `TypeRelation`
- [ ] Handle both JS and TS (TS has type annotations → better resolution)
- [ ] Call resolution Level 0 + Level 1 where types available

### 3.2 Testing
- [ ] Test fixtures: JS and TS files
- [ ] Unit tests
- [ ] Integration test

**Deliverable**: Java + JS/TS + Rust support.

---

## Phase 4 — Kotlin Parser

### 4.1 Kotlin Parser
- [ ] Add `tree-sitter-kotlin` dependency
- [ ] Parse: `class`, `interface`, `object`, `data class`, `sealed class`
- [ ] Extract `val`/`var` properties
- [ ] Extract `fun` methods
- [ ] Extract visibility (`public`, `private`, `protected`, `internal`)
- [ ] Extract `: SuperClass`, `: Interface` → `TypeRelation`
- [ ] Extract annotations
- [ ] Call resolution Level 0 + Level 1

### 4.2 Testing
- [ ] Test fixtures: Kotlin files
- [ ] Unit tests
- [ ] Integration test

**Deliverable**: All 4 languages supported.

---

## Phase 5 — CLI Polish + Call Resolution Level 2

### 5.1 CLI enhancements
- [ ] `--exclude` / `--include` glob filters
- [ ] `--no-calls` flag
- [ ] `--verbose` flag
- [ ] `--output-mermaid` / `--output-machine` separate output files
- [ ] Better error messages and progress reporting

### 5.2 Call Resolution — Level 1 full (Phase 2 of resolution plan)
- [ ] Resolve calls on local variables with explicit type annotations
- [ ] Resolve constructor-assigned variables (`new Foo()` / `Foo::new()`)
- [ ] Single-level return type chaining (`getX().doY()`)

### 5.3 Call Resolution — Level 2 (Phase 3 of resolution plan)
- [ ] Parse import/use statements per file
- [ ] Map type names to fully qualified module paths
- [ ] Cross-module call graph edges

### 5.4 Mermaid Scaling
- [ ] Split large diagrams by module
- [ ] Cross-module relationship summary section

### 5.5 Testing
- [ ] End-to-end tests with multi-language projects
- [ ] Call resolution accuracy tests
- [ ] Large project stress test

**Deliverable**: Production-ready tool.

---

## Progress Tracker

| Phase | Status | Notes |
|---|---|---|
| Phase 1 — Foundation + Rust | Not started | |
| Phase 2 — Java | Not started | |
| Phase 3 — JS/TS | Not started | |
| Phase 4 — Kotlin | Not started | |
| Phase 5 — Polish | Not started | |
