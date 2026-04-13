# Output Formats

Skelecode produces two output formats from the same unified IR. Both formats contain the same structural information, optimized for different consumers.

## Obsidian Vault Format

### Purpose

Human-friendly, interactive knowledge graph for browsing large codebases. Instead of a single static diagram, Skelecode generates a "Vault" folder that uses Obsidian's native Graph View and WikiLinks to map relationships.

### Vault Structure

| File/Folder | Description |
|---|---|
| `Index.md` | Root file listing all modules and entry points. |
| `modules/` | Directory containing one `.md` file per project module. |
| `types/` | Directory containing one `.md` file per type (struct, class, enum). |
| `Topology.canvas` | Native Obsidian Canvas file for visual architecture mapping. |

### Semantic Links (Dataview/Juggl)

Skelecode uses **Inline Fields** (`key:: value`) compatible with plugins like **Dataview** and **Juggl** to label relationships on the graph:

- `defines:: [[Type]]` — Module contains a type definition.
- `contains:: [[Module]]` — Project contains a module.
- `member-of:: [[Module]]` — Type belongs to a module.
- `extends:: [[Type]]`, `implements:: [[Type]]` — Inheritance / Trait mapping.
- `calls:: [[Type|Type::Method]]` — Method-level forward call.
- `called-by:: [[Type|Type::Method]]` — Method-level reverse call.
- `edge-label:: "label"` — Hint for the **Juggl** plugin (e.g. "calls", "called-by").

### Metadata & Tags

Every note includes YAML frontmatter with tags for automatic color-coding in the Obsidian Graph:

```markdown
---
tags:
  - type
  - struct
kind: "struct"
name: "Parser"
module: "src/parser"
language: "rust"
visibility: "pub"
---

# struct Parser

- member-of:: [[src_parser|src/parser (module)]]
- kind:: struct
- visibility:: pub

## Fields

| Name | Type | Visibility |
|------|------|------------|
| `source` | `String` | pub |
| `pos` | `usize` | private |

## Methods

### `new(String)` `static`
**Returns:** `Self`

### `parse()`
**Returns:** `Result<AST>`

**Calls:**
- calls:: [[lexer_Token|Lexer::tokenize]]
  - edge-label:: "calls"
- calls:: [[ast_AST|AST::new]]
  - edge-label:: "calls"

**Called by:**
- called-by:: [[parser_mod|Parser::run]]
```

---

## Obsidian Canvas (Visual Topology)

### Purpose

A native, interactive board (`Topology.canvas`) within the Obsidian vault that provides a pre-laid-out visual map of the project's architecture with **labeled arrows**.

### Features

- **Grid Layout**: Modules are arranged in a multi-column grid.
- **Grouping**: Types are visually grouped inside their respective module containers.
- **Labeled Edges**: Every relationship arrow is explicitly labeled (e.g., "calls", "defines") for immediate clarity without hovering.
- **Color Coding**:
    - **Red**: Classes / Objects
    - **Green**: Structs
    - **Purple**: Traits / Interfaces
    - **Orange**: Enums + Call relationships
    - **Cyan**: Modules

---

## Machine Context Format

### Purpose

Ultra-compact, token-optimized format designed for LLM consumption. Preserves all structural and relational information in minimal tokens.

### Syntax Reference

```
@tag value
```

All tags use the `@` prefix. Indentation indicates nesting (2 spaces per level).

### Tags

| Tag | Scope | Description | Example |
|---|---|---|---|
| `@lang` | top | Language identifier | `@lang java` |
| `@mod` | top | Module/package path | `@mod com.example.service` |
| `@pkg` | top | Alias for `@mod` (Java/Kotlin convention) | `@pkg com.example.service` |
| `@file` | top | File path (JS/TS where modules = files) | `@file src/utils/parser.js` |
| `@type` | top | Type definition with kind | `@type UserService [class]` |
| `@vis` | nested | Visibility modifier | `@vis public` |
| `@field` | nested | Field (alternative to inline `{}` syntax) | `@field repo:UserRepository` |
| `@fn` | nested/top | Method or function | `@fn findUser(Long)->Optional<User>` |
| `@ext` | nested | Extends (inheritance) | `@ext AbstractService` |
| `@impl` | nested | Implements interface/trait | `@impl Serializable` |
| `@static` | suffix | Static method/field | `@static` |
| `@calls` | suffix | Direct forward calls | `@calls[repo.save, cache.invalidate]` |
| `@callers` | suffix | Direct reverse calls (incoming) | `@callers[Service::process]` |
| `@enum` | nested | Enum variants | `@enum Red, Green, Blue` |

### Full Example — Java

```
@lang java
@pkg com.example.service
@type UserService [class] {repo:UserRepository, cache:CacheManager}
  @vis public
  @ext AbstractService
  @impl Serializable
  @ann @Service, @Transactional
  @fn findUser(Long)->Optional<User> @calls[repo.findById, cache.get]
  @fn saveUser(User)->User @calls[repo.save, cache.invalidate, validateUser]
  @fn validateUser(User)->boolean @vis private

@pkg com.example.repository
@type UserRepository [interface]
  @vis public
  @ann @Repository
  @fn findById(Long)->Optional<User>
  @fn save(User)->User
  @fn deleteById(Long)->void
```

### Full Example — JavaScript/TypeScript

```
@lang js
@file src/utils/parser.js
@fn parseConfig(string)->Config @export @calls[validate, normalize]
@fn validate(object)->boolean @calls[checkSchema]
@fn normalize(Config)->Config

@file src/models/token-stream.js
@type TokenStream [class] {tokens:Token[], pos:number}
  @export
  @fn next()->Token
  @fn peek()->Token @calls[this.next]
  @fn collect()->Token[] @calls[this.next]
```

### Full Example — Kotlin

```
@lang kotlin
@pkg com.example.api
@type UserController [class] {service:UserService}
  @vis public
  @ann @RestController, @RequestMapping("/api/users")
  @fn getUser(Long)->ResponseEntity<User> @ann @GetMapping("/{id}") @calls[service.findUser]
  @fn createUser(UserDto)->ResponseEntity<User> @ann @PostMapping @calls[service.saveUser]

@pkg com.example.model
@type User [data class] {id:Long, name:String, email:String}
  @vis public
  @impl Serializable
```

### Full Example — Rust

```
@lang rust
@mod parser
@type Parser [struct] {source:String, pos:usize}
  @vis pub
  @fn new(String)->Self @static
  @fn parse(&self)->Result<AST> @calls[Lexer::tokenize, AST::new]
  @fn expect_token(&self, TokenKind)->Result<Token>
  @impl Display
  @impl Debug

@mod ast
@type AST [enum]
  @vis pub
  @enum Expr(Expr), Stmt(Stmt)

@type Expr [struct] {kind:ExprKind, span:Span}
  @vis pub
  @fn new(ExprKind, Span)->Self @static

@mod lexer
@fn tokenize(&str)->Vec<Token> @vis pub @calls[Token::new, classify_char]
@type Token [struct] {kind:TokenKind, span:Span, text:String}
  @vis pub

### Full Example — Python

```python
@lang python
@mod src.models.user
@type User [class] {name:str, _age:int}
  @vis public
  @fn greet()->str @calls[this.name] @callers[AuthService::verify]

@mod src.services.auth
@type AuthService [class]
  @vis public
  @fn verify(User)->boolean @calls[src.models.user::User::greet]
```
```

### Qualified Type Aliases

Large Java/Kotlin projects produce machine context files where the same qualified type references (e.g. `com.scientia.commonserver.dao::DAOFactory`) repeat thousands of times. The alias system compresses these by generating a deterministic short identifier for each high-frequency qualified type, emitted in a header block at the top of the output.

#### Header Format

```
@aliases
$a3b9x = com.example.dao::UserDAO
$k7f2m = com.example.service::UserService
$p1qw4 = org.hibernate.criterion::Restrictions
@end

@lang java
@pkg com.example.controller
@type UserController [class]
  @vis pub
  @fn getUser(id:Long)->User @vis pub @calls[$a3b9x::findById, $k7f2m::validate]
```

#### Alias Rules

| Rule | Detail |
|---|---|
| **Scope** | Only qualified types (containing `::` with a dot-based package prefix) are candidates |
| **Threshold** | A type is aliased only when byte savings in the body exceed the header entry cost |
| **Format** | `$` + 5 base-36 chars = 6 chars total (e.g. `$k7f2m`) |
| **Determinism** | Alias = `FNV-1a(full_type_string)` → base36. Same type → same alias across all projects |
| **Collision** | Alphabetically first type wins. With 60M+ address space, collisions are <0.1% for 10K entries |
| **Definition sites** | `@pkg` lines, `@type` names, `@ext`, `@impl` are NOT aliased — only references in `@calls`/`@callers` |

#### Cross-Project Consistency

When multiple independent projects (e.g. `citadel`, `datamanager`) reference the same shared library type, the alias is identical in all outputs because it is computed solely from the type string, not from the project-specific alias table ordering.

#### Impact on Large Codebases

Measured on a real-world 14MB Java monolith context:

| Metric | Value |
|---|---|
| Unique aliasable types | ~5,600 |
| Total references replaced | ~135,000 |
| File size reduction | **~45%** (14MB → ~7.7MB) |
| Alias header overhead | ~220KB |

### Design Principles

1. **Minimal tokens** — no filler words, no redundant syntax. Every character carries meaning
2. **Scannable** — both AI and humans can read it. `@` tags create natural visual anchors
3. **Extensible** — new `@` tags can be added without breaking existing parsers
4. **Language-unified** — same tag vocabulary across Java, JS, Kotlin, Rust. Language-specific concepts are mapped to common tags
5. **One line per concept** — each method, field, or relationship is at most one line. AI can quickly jump to what it needs
6. **Direct calls only** — transitive call paths are derived from the graph, not duplicated

### Token Comparison

For a typical Java class (~200 lines of source code):

| Representation | Estimated tokens |
|---|---|
| Raw source code | ~800-1200 |
| Machine Context | ~50-100 |
| **Reduction** | **~90-95%** |
