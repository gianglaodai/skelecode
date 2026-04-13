use std::collections::HashMap;

use super::Renderer;
use crate::ir::*;

// ─── Alias System ─────────────────────────────────────────────────────────────
//
// Qualified Java/Kotlin type references (e.g. `com.scientia.commonserver.dao::DAOFactory`)
// are long, repetitive, and dominate file size in large monolith projects.
// The alias system replaces frequently-occurring qualified types with short
// deterministic identifiers (`$xxxxx`), prefixed with a lookup header.
//
// Determinism guarantee: the alias for a given type string is derived solely from
// FNV-1a(type_name) → base36. The same type always maps to the same alias
// regardless of which project includes it.  Collision ties are broken by
// alphabetical ordering of the original name, so both projects that share a
// type will agree on who keeps the primary alias.

/// FNV-1a 64-bit hash — stable, platform-independent, no external crate needed.
fn fnv1a_hash(s: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

/// Fixed-width base-36 encoding of a u64.
fn to_base36(mut n: u64, len: usize) -> String {
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut result = vec![b'0'; len];
    for i in (0..len).rev() {
        result[i] = CHARS[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8(result).expect("base36 chars are ASCII")
}

/// Returns `true` if the string is a qualified Java/Kotlin type reference,
/// i.e. contains `::` with a dot-based package prefix before it.
fn is_qualified_type(s: &str) -> bool {
    match s.find("::") {
        Some(idx) => s[..idx].contains('.'),
        None => false,
    }
}

const ALIAS_BASE36_LEN: usize = 5; // 36^5 ≈ 60M — collision prob <0.1% for 10K entries
const ALIAS_TOTAL_LEN: usize = ALIAS_BASE36_LEN + 1; // "$" prefix

/// Lookup table mapping qualified type strings to short aliases.
struct AliasTable {
    map: HashMap<String, String>,
    /// (alias, original) pairs sorted by original name for the header block.
    entries: Vec<(String, String)>,
}

impl AliasTable {
    /// An empty table that never aliases anything.
    fn empty() -> Self {
        AliasTable {
            map: HashMap::new(),
            entries: Vec::new(),
        }
    }

    /// Scan the project and build the alias table.
    fn build(project: &Project) -> Self {
        // Phase 1: count how often each qualified type appears in calls/callers
        let mut counts: HashMap<String, usize> = HashMap::new();
        for module in &project.modules {
            for td in &module.types {
                for m in &td.methods {
                    count_refs(&m.calls, &m.callers, &mut counts);
                }
            }
            for f in &module.functions {
                count_refs(&f.calls, &f.callers, &mut counts);
            }
        }

        if counts.is_empty() {
            return Self::empty();
        }

        // Phase 2: generate candidate aliases (only where savings > overhead)
        let mut candidates: Vec<(String, String)> = Vec::new();
        for (type_name, count) in &counts {
            // Optimization: NEVER alias types that already have a shorthand (like List, String, etc.)
            let shorthanded = apply_shorthand(type_name);
            if shorthanded.len() < 3 && shorthanded != *type_name {
                continue;
            }

            let header_cost = type_name.len() + ALIAS_TOTAL_LEN + 4; // "$xxxxx = original\n"
            let body_savings = type_name.len().saturating_sub(ALIAS_TOTAL_LEN) * count;
            if body_savings > header_cost {
                let hash = fnv1a_hash(type_name);
                let alias = format!("${}", to_base36(hash, ALIAS_BASE36_LEN));
                candidates.push((type_name.clone(), alias));
            }
        }

        // Phase 3: sort by type name — deterministic collision resolution
        // (alphabetically first type wins the alias)
        candidates.sort_by(|a, b| a.0.cmp(&b.0));

        let mut map = HashMap::new();
        let mut used: HashMap<String, String> = HashMap::new(); // alias → type_name

        for (type_name, alias) in candidates {
            if used.contains_key(&alias) {
                // Collision: later type (alphabetically) loses — stays unaliased.
                // With 5 base36 chars (60M space) and <10K entries this is <0.1%.
                continue;
            }
            used.insert(alias.clone(), type_name.clone());
            map.insert(type_name, alias);
        }

        // Sorted by original name — groups related types by package
        let mut entries: Vec<(String, String)> = map
            .iter()
            .map(|(orig, alias)| (alias.clone(), orig.clone()))
            .collect();
        entries.sort_by(|a, b| a.1.cmp(&b.1));

        AliasTable { map, entries }
    }

    /// Look up alias for a qualified type string.
    fn get(&self, type_name: &str) -> Option<&str> {
        self.map.get(type_name).map(|s| s.as_str())
    }

    /// Render the `@aliases ... @end` header block. Empty string if no aliases.
    fn render_header(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        let mut out = String::from("@aliases\n");
        for (alias, original) in &self.entries {
            out.push_str(alias);
            out.push_str(" = ");
            out.push_str(original);
            out.push('\n');
        }
        out.push_str("@end\n\n");
        out
    }
}

fn count_refs(
    calls: &[CallRef],
    callers: &[CallerRef],
    counts: &mut HashMap<String, usize>,
) {
    for c in calls {
        if is_noise_call(c) { continue; }
        if let Some(ref t) = c.target_type {
            if is_qualified_type(t) {
                *counts.entry(t.clone()).or_insert(0) += 1;
            }
        }
    }
    for c in callers {
        if is_noise_caller(c) { continue; }
        if let Some(ref t) = c.source_type {
            if is_qualified_type(t) {
                *counts.entry(t.clone()).or_insert(0) += 1;
            }
        }
    }
}

// ─── Noise Reduction System ───────────────────────────────────────────────────

fn is_noise_call(call: &CallRef) -> bool {
    let target = call.target_type.as_deref().unwrap_or("");
    let method = call.target_method.as_str();

    // 1. Filter JDK standard libraries
    if target.starts_with("java.lang.")
        || target.starts_with("java.util.")
        || target.starts_with("java.io.")
        || target.starts_with("java.math.")
        || target.starts_with("java.time.")
    {
        return true;
    }

    // 2. Filter primitive & common object types
    let common_types = [
        "String", "StringBuilder", "StringBuffer", "Integer", "Long", "Double", "Boolean",
        "List", "ArrayList", "Map", "HashMap", "Set", "HashSet", "Optional", "System",
        "RuntimeException", "Exception", "Date", "Object", "Collection",
    ];
    if common_types.contains(&target) {
        return true;
    }

    // 3. Filter common utility and life-cycle methods
    let common_methods = [
        "<init>", "toString", "hashCode", "equals", "length", "size", "isEmpty", "trim",
        "append", "printStackTrace", "getMessage", "println", "print", "valueOf", "format",
        "iterator", "hasNext", "next",
    ];
    if common_methods.contains(&method) {
        return true;
    }

    // 4. Filter Getters and Setters (e.g. getEmail, setActive, isReady)
    let is_getter = method.starts_with("get")
        && method.len() > 3
        && method.chars().nth(3).map_or(false, |c| c.is_ascii_uppercase());
    
    let is_setter = method.starts_with("set")
        && method.len() > 3
        && method.chars().nth(3).map_or(false, |c| c.is_ascii_uppercase());

    let is_is = method.starts_with("is")
        && method.len() > 2
        && method.chars().nth(2).map_or(false, |c| c.is_ascii_uppercase());

    if is_getter || is_setter || is_is {
        return true;
    }

    false
}

fn is_noise_caller(caller: &CallerRef) -> bool {
    let source = caller.source_type.as_deref().unwrap_or("");
    let method = caller.source_method.as_str();

    // For callers, we generally use identical heuristics, 
    // excluding standard types/libraries where backwards parsing happened.
    // However, since CallerRef and CallRef are structured similarly, we can fake a CallRef.
    let call = CallRef {
        target_type: Some(source.to_string()),
        target_method: method.to_string(),
    };
    is_noise_call(&call)
}


// ─── Formatting helpers ───────────────────────────────────────────────────────

/// Apply shorthands to common primitive/standard types to save space.
fn apply_shorthand(t: &str) -> String {
    // We use a simple replacement but handle word boundaries for safety.
    // In these IR strings, types are often standalone or followed by '<'.
    let mut s = t.to_string();
    
    let mappings = [
        ("java.util::List", "L"),
        ("java.util::Set", "S"),
        ("java.util::Map", "M"),
        ("java.lang::String", "s"),
        ("java.lang::Integer", "I"),
        ("java.lang::Long", "Lo"),
        ("java.lang::Boolean", "B"),
        ("java.lang::Double", "D"),
        ("String", "s"),
        ("Integer", "I"),
        ("int", "i"),
        ("Long", "Lo"),
        ("long", "lo"), 
        ("List", "L"),
        ("Set", "S"),
        ("Map", "M"),
        ("Boolean", "B"),
        ("boolean", "b"),
        ("Double", "D"),
        ("double", "d"),
        ("void", "v"),
    ];

    for (old, new) in mappings {
        // Replace as whole word or before generic bracket
        if s == old {
            s = new.to_string();
        } else {
            // e.g., "List<String>" -> "L<s>"
            s = s.replace(&format!("{}<", old), &format!("{}<", new))
                 .replace(&format!("<{}", old), &format!("<{}", new))
                 .replace(&format!("{},", old), &format!("{},", new))
                 .replace(&format!(" {}", old), &format!(" {}", new))
                 .replace(&format!("{}>", old), &format!("{}>", new));
        }
    }
    s
}

/// Render the `@shorthands ... @end` block for documentation.
fn render_shorthands_header() -> String {
    let mut out = String::from("@shorthands\n");
    out.push_str("s = String\n");
    out.push_str("I = Integer\n");
    out.push_str("i = int\n");
    out.push_str("L = List\n");
    out.push_str("S = Set\n");
    out.push_str("M = Map\n");
    out.push_str("Lo = Long\n");
    out.push_str("lo = long\n");
    out.push_str("B = Boolean\n");
    out.push_str("b = boolean\n");
    out.push_str("D = Double\n");
    out.push_str("d = double\n");
    out.push_str("v = void\n");
    out.push_str("@end\n\n");
    out
}

/// Sanitize a call/caller string for use inside `@calls[...]` / `@callers[...]`.
/// Replaces `[` → `(` and `]` → `)` so that subscript expressions in
/// Kotlin/JS (e.g. `params[KEY]`) do not prematurely close the bracket list.
fn sanitize_call(s: &str) -> String {
    s.replace('[', "(").replace(']', ")")
}

fn format_call_ref(call: &CallRef, aliases: &AliasTable) -> String {
    if let Some(ref t) = call.target_type {
        if let Some(alias) = aliases.get(t) {
            return sanitize_call(&format!("{}::{}", alias, call.target_method));
        }
        // Fallback to shorthand if no alias was assigned (e.g. for List, Set, Map)
        let short_t = apply_shorthand(t);
        if short_t != *t {
            return sanitize_call(&format!("{}::{}", short_t, call.target_method));
        }
    }
    sanitize_call(&format!("{}", call))
}

fn format_caller_ref(caller: &CallerRef, aliases: &AliasTable) -> String {
    if let Some(ref t) = caller.source_type {
        if let Some(alias) = aliases.get(t) {
            return sanitize_call(&format!("{}::{}", alias, caller.source_method));
        }
        // Fallback to shorthand if no alias was assigned
        let short_t = apply_shorthand(t);
        if short_t != *t {
            return sanitize_call(&format!("{}::{}", short_t, caller.source_method));
        }
    }
    sanitize_call(&format!("{}", caller))
}

// ─── Renderer ─────────────────────────────────────────────────────────────────

pub struct MachineRenderer;

impl Renderer for MachineRenderer {
    fn render(&self, project: &Project) -> crate::renderer::RenderOutput {
        let aliases = AliasTable::build(project);
        let mut out = render_shorthands_header();
        out.push_str(&aliases.render_header());

        // Group modules by path (e.g., lib.rs + main.rs both yield "crate"),
        // preserving first-seen order. Skip groups with no content.
        let mut seen_paths: Vec<String> = Vec::new();
        let mut groups: Vec<Vec<&Module>> = Vec::new();

        for module in &project.modules {
            if let Some(pos) = seen_paths.iter().position(|p| p == &module.path) {
                groups[pos].push(module);
            } else {
                seen_paths.push(module.path.clone());
                groups.push(vec![module]);
            }
        }

        let mut first = true;
        for (path, modules) in seen_paths.iter().zip(groups.iter()) {
            // Skip empty groups
            let has_content = modules
                .iter()
                .any(|m| !m.types.is_empty() || !m.functions.is_empty());
            if !has_content {
                continue;
            }

            if !first {
                out.push('\n');
            }
            first = false;

            render_module_group(path, modules, &aliases, &mut out);
        }

        crate::renderer::RenderOutput::Single(out)
    }
}

/// Render a group of modules sharing the same path (merged into one block).
fn render_module_group(path: &str, modules: &[&Module], aliases: &AliasTable, out: &mut String) {
    // Determine the lang tag from the first module that has content
    let lang = modules
        .iter()
        .map(|m| m.language.as_str())
        .next()
        .unwrap_or("rust");
    out.push_str(&format!("@lang {}\n", lang));

    let mod_tag = match modules[0].language {
        Language::Java | Language::Kotlin => "@pkg",
        Language::JavaScript => "@file",
        Language::Rust | Language::Python => "@mod",
    };
    out.push_str(&format!("{} {}\n", mod_tag, path));

    for module in modules {
        for td in &module.types {
            render_type(td, aliases, out);
        }
        for func in &module.functions {
            render_function(func, aliases, out);
        }
    }
}

fn render_type(td: &TypeDef, aliases: &AliasTable, out: &mut String) {
    // @type Name [kind] {fields}
    let mut line = format!("@type {} [{}]", td.name, td.kind.as_str());

    // Inline type params
    if !td.type_params.is_empty() {
        line.push_str(&format!(" @gen <{}>", td.type_params.join(", ")));
    }

    // Inline fields for compact representation
    if !td.fields.is_empty() {
        let fields: Vec<String> = td
            .fields
            .iter()
            .map(|f| format!("{}:{}", f.name, apply_shorthand(&f.type_name)))
            .collect();
        line.push_str(&format!(" {{{}}}", fields.join(", ")));
    }

    out.push_str(&line);
    out.push('\n');

    // Visibility
    if td.visibility != Visibility::Public {
        out.push_str(&format!("  ~{}\n", td.visibility.as_str()));
    }

    // Enum variants
    if !td.enum_variants.is_empty() {
        out.push_str(&format!("  @enum {}\n", td.enum_variants.join(", ")));
    }

    // Annotations
    if !td.annotations.is_empty() {
        let anns: Vec<String> = td.annotations.iter().map(|a| a.name.clone()).collect();
        out.push_str(&format!("  @{}\n", anns.join(", ")));
    }

    // Relations
    for rel in &td.relations {
        match rel.kind {
            RelationKind::Extends => out.push_str(&format!("  @ext {}\n", rel.target)),
            RelationKind::Implements | RelationKind::ImplTrait => {
                out.push_str(&format!("  @impl {}\n", rel.target));
            }
        }
    }

    // Methods
    for method in &td.methods {
        render_method(method, aliases, out);
    }
}

fn render_method(method: &Method, aliases: &AliasTable, out: &mut String) {
    let params: Vec<String> = method
        .params
        .iter()
        .map(|p| {
            if p.name.is_empty() || p.name == "_" {
                apply_shorthand(&p.type_name)
            } else {
                format!("{}:{}", p.name, apply_shorthand(&p.type_name))
            }
        })
        .collect();
    let mut line = format!("  @fn {}({})", method.name, params.join(", "));

    if let Some(ref ret) = method.return_type {
        let short_ret = apply_shorthand(ret);
        if short_ret != "v" && short_ret != "()" && short_ret != "Unit" {
            line.push_str(&format!("->{}", short_ret));
        }
    }

    if method.visibility != Visibility::Public {
        line.push_str(&format!(" ~{}", method.visibility.as_str()));
    }

    if method.is_static {
        line.push_str(" *");
    }

    if !method.annotations.is_empty() {
        let anns: Vec<String> = method.annotations.iter().map(|a| a.name.clone()).collect();
        line.push_str(&format!(" @{}", anns.join(", ")));
    }

    let filtered_calls: Vec<_> = method.calls.iter().filter(|c| !is_noise_call(c)).collect();
    if !filtered_calls.is_empty() {
        let calls: Vec<String> = filtered_calls
            .into_iter()
            .map(|c| format_call_ref(c, aliases))
            .collect();
        line.push_str(&format!(" >[{}]", calls.join(", ")));
    }

    let filtered_callers: Vec<_> = method.callers.iter().filter(|c| !is_noise_caller(c)).collect();
    if !filtered_callers.is_empty() {
        let callers: Vec<String> = filtered_callers
            .into_iter()
            .map(|c| format_caller_ref(c, aliases))
            .collect();
        line.push_str(&format!(" <[{}]", callers.join(", ")));
    }

    out.push_str(&line);
    out.push('\n');
}

fn render_function(func: &Function, aliases: &AliasTable, out: &mut String) {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| {
            if p.name.is_empty() || p.name == "_" {
                apply_shorthand(&p.type_name)
            } else {
                format!("{}:{}", p.name, apply_shorthand(&p.type_name))
            }
        })
        .collect();
    let mut line = format!("@fn {}({})", func.name, params.join(", "));

    if let Some(ref ret) = func.return_type {
        let short_ret = apply_shorthand(ret);
        if short_ret != "v" && short_ret != "()" && short_ret != "Unit" {
            line.push_str(&format!("->{}", short_ret));
        }
    }

    if func.visibility != Visibility::Public {
        line.push_str(&format!(" ~{}", func.visibility.as_str()));
    }

    let filtered_calls: Vec<_> = func.calls.iter().filter(|c| !is_noise_call(c)).collect();
    if !filtered_calls.is_empty() {
        let calls: Vec<String> = filtered_calls
            .into_iter()
            .map(|c| format_call_ref(c, aliases))
            .collect();
        line.push_str(&format!(" >[{}]", calls.join(", ")));
    }

    let filtered_callers: Vec<_> = func.callers.iter().filter(|c| !is_noise_caller(c)).collect();
    if !filtered_callers.is_empty() {
        let callers: Vec<String> = filtered_callers
            .into_iter()
            .map(|c| format_caller_ref(c, aliases))
            .collect();
        line.push_str(&format!(" <[{}]", callers.join(", ")));
    }

    out.push_str(&line);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_project() -> Project {
        Project {
            modules: vec![Module {
                path: "parser".to_string(),
                language: Language::Rust,
                types: vec![TypeDef {
                    name: "Parser".to_string(),
                    kind: TypeKind::Struct,
                    visibility: Visibility::Public,
                    fields: vec![
                        Field {
                            name: "source".to_string(),
                            type_name: "String".to_string(),
                            visibility: Visibility::Private,
                        },
                        Field {
                            name: "pos".to_string(),
                            type_name: "usize".to_string(),
                            visibility: Visibility::Private,
                        },
                    ],
                    methods: vec![
                        Method {
                            name: "new".to_string(),
                            params: vec![Param {
                                name: "source".to_string(),
                                type_name: "String".to_string(),
                            }],
                            return_type: Some("Self".to_string()),
                            visibility: Visibility::Public,
                            calls: Vec::new(),
                            callers: Vec::new(),
                            annotations: Vec::new(),
                            is_static: true,
                        },
                        Method {
                            name: "parse".to_string(),
                            params: Vec::new(),
                            return_type: Some("Result<AST>".to_string()),
                            visibility: Visibility::Public,
                            calls: vec![
                                CallRef {
                                    target_type: Some("Lexer".to_string()),
                                    target_method: "tokenize".to_string(),
                                },
                                CallRef {
                                    target_type: Some("AST".to_string()),
                                    target_method: "new".to_string(),
                                },
                            ],
                            callers: Vec::new(),
                            annotations: Vec::new(),
                            is_static: false,
                        },
                    ],
                    relations: vec![TypeRelation {
                        kind: RelationKind::ImplTrait,
                        target: "Display".to_string(),
                    }],
                    annotations: Vec::new(),
                    type_params: Vec::new(),
                    enum_variants: Vec::new(),
                }],
                functions: vec![Function {
                    name: "helper".to_string(),
                    params: Vec::new(),
                    return_type: None,
                    visibility: Visibility::Private,
                    calls: Vec::new(),
                    callers: Vec::new(),
                }],
                imports: Vec::new(),
            }],
        }
    }

    #[test]
    fn test_machine_output() {
        let renderer = MachineRenderer;
        let output = match renderer.render(&sample_project()) {
            crate::renderer::RenderOutput::Single(out) => out,
            _ => panic!("Expected single output"),
        };

        assert!(output.contains("@shorthands"));
        assert!(output.contains("s = String"));
        assert!(output.contains("@lang rust"));
        assert!(output.contains("@mod parser"));
        assert!(output.contains("@type Parser [struct] {source:s, pos:usize}"));
        assert!(!output.contains("~pub"));
        assert!(output.contains("@impl Display"));
        assert!(output.contains("@fn new(source:s)->Self *"));
        assert!(
            output.contains("@fn parse()->Result<AST> >[Lexer::tokenize, AST::new]")
        );
        assert!(output.contains("@fn helper() ~private"));
        // Rust project → no qualified Java types → no alias header
        assert!(!output.contains("@aliases"));
    }

    /// Build a Java project where `com.example.dao::MyDAO` is referenced 5 times
    /// in calls — enough to exceed the alias threshold.
    fn java_project_with_repeated_calls() -> Project {
        let dao_type = "com.example.dao::MyDAO".to_string();
        let util_type = "com.example.util::StringUtils".to_string();
        Project {
            modules: vec![Module {
                path: "com.example.service".to_string(),
                language: Language::Java,
                types: vec![TypeDef {
                    name: "OrderService".to_string(),
                    kind: TypeKind::Class,
                    visibility: Visibility::Public,
                    fields: Vec::new(),
                    methods: vec![
                        Method {
                            name: "createOrder".to_string(),
                            params: Vec::new(),
                            return_type: Some("void".to_string()),
                            visibility: Visibility::Public,
                            calls: vec![
                                CallRef { target_type: Some(dao_type.clone()), target_method: "save".to_string() },
                                CallRef { target_type: Some(dao_type.clone()), target_method: "findAll".to_string() },
                                CallRef { target_type: Some(util_type.clone()), target_method: "trim".to_string() },
                            ],
                            callers: Vec::new(),
                            annotations: Vec::new(),
                            is_static: false,
                        },
                        Method {
                            name: "deleteOrder".to_string(),
                            params: Vec::new(),
                            return_type: Some("void".to_string()),
                            visibility: Visibility::Public,
                            calls: vec![
                                CallRef { target_type: Some(dao_type.clone()), target_method: "delete".to_string() },
                                CallRef { target_type: Some(dao_type.clone()), target_method: "flush".to_string() },
                                CallRef { target_type: Some(util_type.clone()), target_method: "format".to_string() },
                            ],
                            callers: Vec::new(),
                            annotations: Vec::new(),
                            is_static: false,
                        },
                        Method {
                            name: "getOrder".to_string(),
                            params: Vec::new(),
                            return_type: Some("Order".to_string()),
                            visibility: Visibility::Public,
                            calls: vec![
                                CallRef { target_type: Some(dao_type.clone()), target_method: "findById".to_string() },
                            ],
                            callers: vec![
                                CallerRef { source_type: Some(util_type.clone()), source_method: "helper".to_string() },
                            ],
                            annotations: Vec::new(),
                            is_static: false,
                        },
                    ],
                    relations: Vec::new(),
                    annotations: Vec::new(),
                    type_params: Vec::new(),
                    enum_variants: Vec::new(),
                }],
                functions: Vec::new(),
                imports: Vec::new(),
            }],
        }
    }

    #[test]
    fn test_alias_header_present_for_java() {
        let project = java_project_with_repeated_calls();
        let output = match MachineRenderer.render(&project) {
            crate::renderer::RenderOutput::Single(out) => out,
            _ => panic!("Expected single output"),
        };

        // Alias header should be present and well-formed
        assert!(output.starts_with("@aliases\n"));
        assert!(output.contains("@end\n"));

        // The original qualified name appears in the header (as the definition)
        assert!(output.contains("com.example.dao::MyDAO"));

        // The body should NOT contain the full qualified name in @calls
        // (it should use the alias instead)
        let body = output.split("@end\n").nth(1).unwrap();
        assert!(
            !body.contains("com.example.dao::MyDAO::"),
            "body should use alias, not full qualified name"
        );

        // Method names should still be visible in the body
        assert!(body.contains("::save"));
        assert!(body.contains("::findAll"));
        assert!(body.contains("::delete"));
        assert!(body.contains("::findById"));
    }

    #[test]
    fn test_alias_is_deterministic() {
        // Same type → same hash → same alias, always
        let h1 = fnv1a_hash("com.example.dao::MyDAO");
        let h2 = fnv1a_hash("com.example.dao::MyDAO");
        assert_eq!(h1, h2);

        let a1 = to_base36(h1, ALIAS_BASE36_LEN);
        let a2 = to_base36(h2, ALIAS_BASE36_LEN);
        assert_eq!(a1, a2);

        // Different types → (almost certainly) different aliases
        let h3 = fnv1a_hash("com.other.pkg::OtherType");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_alias_not_applied_below_threshold() {
        // A type referenced only once should NOT be aliased
        let project = Project {
            modules: vec![Module {
                path: "com.example.app".to_string(),
                language: Language::Java,
                types: vec![TypeDef {
                    name: "App".to_string(),
                    kind: TypeKind::Class,
                    visibility: Visibility::Public,
                    fields: Vec::new(),
                    methods: vec![Method {
                        name: "run".to_string(),
                        params: Vec::new(),
                        return_type: None,
                        visibility: Visibility::Public,
                        calls: vec![CallRef {
                            target_type: Some("com.example.dao::MyDAO".to_string()),
                            target_method: "init".to_string(),
                        }],
                        callers: Vec::new(),
                        annotations: Vec::new(),
                        is_static: false,
                    }],
                    relations: Vec::new(),
                    annotations: Vec::new(),
                    type_params: Vec::new(),
                    enum_variants: Vec::new(),
                }],
                functions: Vec::new(),
                imports: Vec::new(),
            }],
        };

        let output = match MachineRenderer.render(&project) {
            crate::renderer::RenderOutput::Single(out) => out,
            _ => panic!("Expected single output"),
        };

        // Only 1 ref → savings < overhead → no alias header
        assert!(!output.contains("@aliases"));
        // Full name should appear directly in body
        assert!(output.contains("com.example.dao::MyDAO::init"));
    }

    #[test]
    fn test_unqualified_types_never_aliased() {
        // Types without dot-based package (e.g. Rust, or same-package Java refs)
        // should never be aliased
        assert!(!is_qualified_type("DAOFactory"));
        assert!(!is_qualified_type("std::vec::Vec"));
        assert!(!is_qualified_type("Lexer"));

        // Only dot-based packages qualify
        assert!(is_qualified_type("com.example.dao::MyDAO"));
        assert!(is_qualified_type("org.hibernate.criterion::Restrictions"));
    }

    #[test]
    fn test_alias_consistency_across_projects() {
        // Two different projects referencing the same type should produce the same alias
        let shared_type = "com.example.common::SharedUtil".to_string();

        let project_a = Project {
            modules: vec![Module {
                path: "com.example.app_a".to_string(),
                language: Language::Java,
                types: vec![TypeDef {
                    name: "ServiceA".to_string(),
                    kind: TypeKind::Class,
                    visibility: Visibility::Public,
                    fields: Vec::new(),
                    methods: vec![Method {
                        name: "work".to_string(),
                        params: Vec::new(),
                        return_type: None,
                        visibility: Visibility::Public,
                        calls: (0..5)
                            .map(|i| CallRef {
                                target_type: Some(shared_type.clone()),
                                target_method: format!("method{}", i),
                            })
                            .collect(),
                        callers: Vec::new(),
                        annotations: Vec::new(),
                        is_static: false,
                    }],
                    relations: Vec::new(),
                    annotations: Vec::new(),
                    type_params: Vec::new(),
                    enum_variants: Vec::new(),
                }],
                functions: Vec::new(),
                imports: Vec::new(),
            }],
        };

        let project_b = Project {
            modules: vec![Module {
                path: "com.example.app_b".to_string(),
                language: Language::Java,
                types: vec![TypeDef {
                    name: "ServiceB".to_string(),
                    kind: TypeKind::Class,
                    visibility: Visibility::Public,
                    fields: Vec::new(),
                    methods: vec![Method {
                        name: "run".to_string(),
                        params: Vec::new(),
                        return_type: None,
                        visibility: Visibility::Public,
                        calls: (0..5)
                            .map(|i| CallRef {
                                target_type: Some(shared_type.clone()),
                                target_method: format!("fn{}", i),
                            })
                            .collect(),
                        callers: Vec::new(),
                        annotations: Vec::new(),
                        is_static: false,
                    }],
                    relations: Vec::new(),
                    annotations: Vec::new(),
                    type_params: Vec::new(),
                    enum_variants: Vec::new(),
                }],
                functions: Vec::new(),
                imports: Vec::new(),
            }],
        };

        let out_a = match MachineRenderer.render(&project_a) {
            crate::renderer::RenderOutput::Single(out) => out,
            _ => panic!("Expected single output"),
        };
        let out_b = match MachineRenderer.render(&project_b) {
            crate::renderer::RenderOutput::Single(out) => out,
            _ => panic!("Expected single output"),
        };

        // Extract the alias assigned to SharedUtil in each project
        let find_alias = |output: &str| -> String {
            for line in output.lines() {
                if line.contains("com.example.common::SharedUtil") && line.contains(" = ") {
                    return line.split(" = ").next().unwrap().trim().to_string();
                }
            }
            panic!("Alias not found for SharedUtil");
        };

        let alias_a = find_alias(&out_a);
        let alias_b = find_alias(&out_b);
        assert_eq!(alias_a, alias_b, "Same type must get same alias across projects");
    }
}
