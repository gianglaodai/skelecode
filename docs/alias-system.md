# Alias System — Design & Roadmap

## Status

- **Phase 1 (done)**: Qualified type aliasing in Machine Context for Java/Kotlin.
- **Phase 2 (planned)**: Extension to other languages and renderers.

## Problem

In large Java/Kotlin monoliths, machine context output is dominated by repeated long qualified type references:

```
com.scientia.commonserver.dao::DAOFactory::getDAO          — 4,215 occurrences
com.scientia.commonserver.dao::DAOFactory::getCurrentInstance — 4,056 occurrences
com.scientia.commonserver.service.rest.resource::ResourceUtils::resolveServiceException — 2,868 occurrences
```

In a real 14MB context file: **60,364 qualified references** across **880 unique packages**. The qualified type portion alone accounts for ~2MB of raw text, but because each full `package::Type::method` call repeats, the total addressable bytes exceed 7MB.

## Current Implementation (Phase 1)

### What Gets Aliased

The **`target_type`** field of `CallRef` and the **`source_type`** field of `CallerRef` — these are the `package::ClassName` strings that appear inside `@calls[...]` and `@callers[...]`.

A type is aliased when:
- It contains `::` with a dot-based package prefix (i.e., Java/Kotlin convention)
- `body_savings > header_cost`, where:
  - `body_savings = (original_length - alias_length) * occurrence_count`
  - `header_cost = original_length + alias_length + 4` (one header line)

### What Is NOT Aliased

| Element | Reason |
|---|---|
| `@pkg` module path | Definition site — must remain readable |
| `@type` name | Local to its block, already short |
| `@ext` / `@impl` targets | Usually unqualified class names |
| Unqualified call targets (e.g. `DAOFactory::save`) | No dot-based package prefix |
| Rust/Python/JS modules | Use `::` but no dots — `std::vec::Vec` is not aliased |

### Alias Generation

```
alias = "$" + base36(FNV-1a_64bit(type_string), 5)
```

- **FNV-1a 64-bit**: chosen for simplicity (no crate dependency), determinism, and uniform distribution.
- **Base-36, 5 chars**: 36^5 = 60,466,176 address space. With <10K aliases, birthday-paradox collision probability is <0.1%.
- **Collision resolution**: candidates are sorted alphabetically by type name; the first type wins. The losing type stays unaliased. This is deterministic regardless of project composition.

### Cross-Project Consistency

The alias is a pure function of the type string:

```
hash("com.scientia.commonserver.dao::DAOFactory") → same alias in citadel, datamanager, or any other project
```

No project-specific state (insertion order, module set) affects the alias, so shared library types get identical aliases across independently-rendered outputs.

## Roadmap (Phase 2+)

### 2a. Package-Level Aliasing (Additive)

Current approach aliases `package::ClassName` as a unit. An additional pass could alias just the **package prefix** for types that don't meet the full-type threshold:

```
@aliases
$p1 = com.scientia.commonserver.service.rest.resource
@end

@calls[$p1::ResourceUtils::resolveServiceException]
```

This would catch types that appear only 1-2 times (below current threshold) but whose packages are long and shared. Estimated additional savings: ~1MB on the 14MB benchmark.

**Design consideration**: package aliases and type aliases should coexist. The renderer should apply the most specific alias available (type alias > package alias).

### 2b. Extension to Rust / Python / JS-TS

Rust's `crate::module::Type` and Python's `package.module::Class` also produce qualified names, but they use different conventions:

| Language | Qualified format | Dot-based? | Current aliasing |
|---|---|---|---|
| Java/Kotlin | `com.example.dao::DAOFactory` | Yes | Aliased |
| Rust | `crate::parser::Parser` | No (uses `::`) | Not aliased |
| Python | `src.models.user::User` | Yes (dots in module) | Would be aliased by current heuristic |
| JS/TS | Usually unqualified or file-path based | Varies | Not aliased |

To support Rust, the `is_qualified_type()` check needs to be broadened. Possible heuristic: alias any `target_type` containing `::` where the prefix is more than N characters, regardless of dots. But this risks aliasing short Rust paths like `std::vec::Vec` where savings are minimal.

**Recommendation**: add a language-aware mode where the qualification check adapts:
- Java/Kotlin: current dot-based heuristic
- Rust: alias if prefix length > 15 chars (e.g. `crate::parser::resolver` but not `Vec`)
- Python: same as Java (dot-based modules)
- JS/TS: skip (paths are usually filesystem-based and short)

### 2c. Extension to Obsidian Renderer

The Obsidian renderer could benefit from aliasing in the Mermaid diagram labels and wiki-link text, but the `[[wikilink]]` syntax requires the full filename stem. Possible approaches:

1. **Abbreviate display text only**: `[[full_path_file|$alias]]` — keeps the link target intact
2. **Skip aliasing in Obsidian** — the vault format is designed for human browsing where full names aid navigation

**Recommendation**: skip for now. Obsidian output is human-oriented; aliases hurt readability there.

### 2d. CLI Control

Future CLI flags for alias behavior:

```
--no-alias          Disable alias system entirely
--alias-threshold N Minimum occurrence count to trigger aliasing (default: auto)
--alias-length N    Base-36 character count (default: 5)
```

### 2e. Callers Compression (Separate Feature)

Aliasing reduces the per-reference cost but doesn't address **caller list explosion** — a single method can have 100+ callers listed inline. This is a separate problem with different solutions:

- **Truncation**: `@callers[A::foo, B::bar, ...+98 more]`
- **Test exclusion**: filter out test classes from `@callers` by annotation (`@Test`) or path heuristic
- **Verbosity levels**: `--detail skeleton|summary|full`

These are complementary to aliasing and should be tracked as separate work items.
