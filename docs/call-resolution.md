# Call Resolution Guide

## The Challenge

Tree-sitter is a syntactic parser — it gives us the AST but no type information. When parsing `repo.findById(id)`, tree-sitter produces:

```
method_invocation
  object: identifier "repo"       ← variable name, NOT the type
  name: identifier "findById"
  arguments: [identifier "id"]
```

To know that `repo.findById` is actually `UserRepository::findById`, we need to resolve the type of `repo` — a job normally done by the compiler, not a parser.

## Resolution Levels

### Level 0 — Raw Syntactic (baseline)

Record calls exactly as they appear in syntax. No type resolution.

```
@fn saveUser(User)->User @calls[repo.save, cache.invalidate, validateUser]
```

- `repo.save` — variable name + method name, type unknown
- `validateUser` — same-class call, implicitly `this`
- **Pro**: trivial to implement, never wrong
- **Con**: AI must infer types from surrounding context

### Level 1 — Heuristic Type Resolution (✅ Implemented)

Use type information already available within the same file/class to resolve variable types.

**Type information sources:**

| Source | Example | Confidence |
|---|---|---|
| Field declarations | `private UserRepository repo;` | High |
| Method parameters | `fn process(parser: &Parser)` | High |
| Local variables with explicit type | `val x: Foo = ...` / `Foo x = ...` | High |
| Constructor calls | `let x = Parser::new()` / `var x = new Parser()` | High |
| Static / type-level calls | `UserService.getInstance()` | 100% |
| `this`/`self` calls | `this.validate()` / `self.parse()` | 100% |
| Return type chaining | `getRepo().save()` — requires knowing return type of `getRepo()` | Medium |

**Example resolution:**

```java
class UserService {
    private UserRepository repo;      // field type known
    private CacheManager cache;       // field type known

    public User saveUser(User user) { // param type known
        repo.save(user);              // → UserRepository::save (from field)
        cache.invalidate(user.getId()); // → CacheManager::invalidate (from field)
        validateUser(user);           // → this::validateUser (same class)
    }
}
```

**Estimated coverage: ~85-90% of calls in a typical codebase.**

### Level 2 — Import-Aware Resolution (✅ Implemented)

Combine Level 1 with import/use statements to resolve fully qualified module paths.

```java
import com.example.repository.UserRepository;
// → UserRepository references resolve to com.example.repository.UserRepository
```

```rust
use crate::parser::Parser;
// → Parser::new() resolves to crate::parser::Parser::new
```

This does not help resolve *more* calls, but enriches already-resolved calls with cross-module path information.

### Level 3 — Reverse Call Graph (✅ Implemented)

After resolving forward calls (Level 1 & 2), Skelecode performs a final graph pass to link targets back to their callers.

- **`CallerRef`**: Each `Method` or `Function` now contains a `callers: Vec<CallerRef>` list.
- **Searchable Architecture**: Enables the "Called by" view in TUI and the `called-by::` back-links in Obsidian.
- **Bi-directional Topology**: Allows the Renderer to draw arrows from target to source in Graph views.

### Level 4 — Full Type Inference (out of scope)

Requires compiler-level analysis: tracking types through `var`/`auto` inference, generic instantiation, trait resolution, etc. This is effectively rebuilding half the compiler for each language.

**Not planned.** The cost-to-benefit ratio is poor for a structural analysis tool.

## Implementation Phases

| Phase | Goal | Status |
|---|---|---|
| **Phase 7** | Local Type Heuristics (this, self, fields) | ✅ Done |
| **Phase 8** | Cross-module Import Resolution | ✅ Done |
| **Phase 10** | Reverse Call Graph (Bi-directional linking) | ✅ Done |
| **Future** | Chaining & Type Alias follow-through | 🛠️ Backlog |

## Unresolved Call Representation

When a call cannot be resolved, the output keeps the raw syntactic form:

```
# Fully resolved
@fn saveUser(User)->User @calls[UserRepository::save, CacheManager::invalidate, this::validateUser]

# Partially resolved (repo type unknown)
@fn process()->void @calls[repo.doSomething, this::validate]
```

The raw variable name (`repo.doSomething`) signals to the reader that this call is unresolved. AI can still infer the type from field declarations elsewhere in the output.

## Explicitly Out of Scope

These cases are not worth solving for a structural analysis tool:

| Case | Example | Why skip |
|---|---|---|
| Dynamic dispatch / polymorphism | `animal.speak()` — Dog or Cat? | Requires runtime information |
| Reflection | `method.invoke(obj, args)` | Cannot be resolved statically |
| Higher-order functions as calls | `list.map(this::transform)` | Complex, low structural value |
| Deep method chaining | `a.b().c().d()` | Requires full type inference |
| JS computed property access | `obj[methodName]()` | Dynamic, cannot resolve |
| Closures / lambdas calling outer scope | `{ callback() }` | Ambiguous target |

These cases represent an estimated ~5-10% of calls in real-world codebases. The trade-off of skipping them (implementation complexity vs. marginal value) is acceptable.

## Language-Specific Notes

### Java

- Field types always explicit → high resolution rate
- Watch for: anonymous classes, lambda expressions, method references (`Class::method`)
- `var` (Java 10+) requires right-hand-side analysis

### JavaScript / TypeScript

- JS: no type annotations on fields/params → resolution mostly limited to constructor patterns and `this` calls
- TS: explicit types available → resolution similar to Java
- Watch for: destructuring, spread operators, prototype-based patterns

### Kotlin

- Explicit types on properties → good resolution
- `val`/`var` with inferred types → requires right-hand-side analysis
- Extension functions: `receiver.extFn()` — need to know receiver type
- Watch for: scope functions (`let`, `apply`, `run`) change what `this`/`it` refers to

### Rust

- `self` methods → 100% resolvable within `impl` block
- `Type::method()` static calls → 100% resolvable
- Watch for: trait method calls (need to know which trait is in scope), closures, `?` operator chaining
- `let x: Type = ...` explicit annotations → resolvable
- `let x = expr` without annotation → needs inference (skip in v1)
