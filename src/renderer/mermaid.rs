use super::Renderer;
use crate::ir::*;

pub struct MermaidRenderer;

impl Renderer for MermaidRenderer {
    fn render(&self, project: &Project) -> String {
        let mut out = String::new();

        // FIX: Group modules by path (e.g., lib.rs + main.rs both yield "crate")
        // and skip groups with no content (empty modules).
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
            // FIX: Skip empty groups (no types and no functions)
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

            render_module_group(path, modules, &mut out);
        }

        out
    }
}

/// Render a group of modules that share the same path (merged into one diagram).
fn render_module_group(path: &str, modules: &[&Module], out: &mut String) {
    out.push_str(&format!("## Module: {}\n\n", path));
    out.push_str("```mermaid\nclassDiagram\n");

    // Render all types from all module files in this group
    for module in modules {
        for td in &module.types {
            render_type(td, out);
        }
    }

    // Merge free functions from all files into one utility class
    let all_fns: Vec<&Function> = modules.iter().flat_map(|m| m.functions.iter()).collect();
    if !all_fns.is_empty() {
        out.push_str(&format!(
            "    class {}__functions {{\n",
            sanitize_id(path)
        ));
        out.push_str("        <<module>>\n");
        for func in &all_fns {
            render_free_function(func, out);
        }
        out.push_str("    }\n");
    }

    // Relationships and call edges
    for module in modules {
        for td in &module.types {
            render_relationships(td, out);
        }
        render_call_edges(module, out);
    }

    out.push_str("```\n");
}

fn render_type(td: &TypeDef, out: &mut String) {
    let id = sanitize_id(&td.name);
    out.push_str(&format!("    class {} {{\n", id));

    // Stereotype
    match td.kind {
        TypeKind::Interface => out.push_str("        <<interface>>\n"),
        TypeKind::Trait => out.push_str("        <<trait>>\n"),
        TypeKind::Enum => out.push_str("        <<enum>>\n"),
        TypeKind::Record => out.push_str("        <<record>>\n"),
        TypeKind::DataClass => out.push_str("        <<data class>>\n"),
        TypeKind::Object => out.push_str("        <<object>>\n"),
        _ => {}
    }

    // Enum variants
    for variant in &td.enum_variants {
        out.push_str(&format!("        {}\n", sanitize_mermaid(variant)));
    }

    // Fields
    for field in &td.fields {
        out.push_str(&format!(
            "        {}{} {}\n",
            field.visibility.mermaid_marker(),
            sanitize_mermaid(&field.type_name),
            field.name,
        ));
    }

    // Methods
    for method in &td.methods {
        let params: Vec<String> = method
            .params
            .iter()
            .map(|p| {
                if p.name.is_empty() {
                    sanitize_mermaid(&p.type_name)
                } else {
                    format!("{} {}", sanitize_mermaid(&p.type_name), p.name)
                }
            })
            .collect();

        let ret = method
            .return_type
            .as_ref()
            .map(|r| format!(" {}", sanitize_mermaid(r)))
            .unwrap_or_default();

        let static_marker = if method.is_static { "$" } else { "" };

        out.push_str(&format!(
            "        {}{}({}){}{}\n",
            method.visibility.mermaid_marker(),
            method.name,
            params.join(", "),
            ret,
            static_marker,
        ));
    }

    out.push_str("    }\n");
}

fn render_free_function(func: &Function, out: &mut String) {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| {
            if p.name.is_empty() {
                sanitize_mermaid(&p.type_name)
            } else {
                format!("{} {}", sanitize_mermaid(&p.type_name), p.name)
            }
        })
        .collect();

    let ret = func
        .return_type
        .as_ref()
        .map(|r| format!(" {}", sanitize_mermaid(r)))
        .unwrap_or_default();

    out.push_str(&format!(
        "        {}{}({}){}\n",
        func.visibility.mermaid_marker(),
        func.name,
        params.join(", "),
        ret,
    ));
}

fn render_relationships(td: &TypeDef, out: &mut String) {
    let id = sanitize_id(&td.name);
    for rel in &td.relations {
        let target_id = sanitize_id(&rel.target);
        match rel.kind {
            RelationKind::Extends => {
                out.push_str(&format!("    {} <|-- {} : extends\n", target_id, id));
            }
            RelationKind::Implements | RelationKind::ImplTrait => {
                out.push_str(&format!("    {} ..|> {} : implements\n", id, target_id));
            }
        }
    }
}

fn render_call_edges(module: &Module, out: &mut String) {
    // Collect unique type-to-type call edges.
    // FIX: filter out non-type targets (local variables, field accesses, etc.)
    let mut edges: Vec<(String, String)> = Vec::new();

    for td in &module.types {
        for method in &td.methods {
            for call in &method.calls {
                if let Some(ref target_type) = call.target_type {
                    // Skip self-references and intra-type calls
                    if target_type == "self" || *target_type == td.name {
                        continue;
                    }
                    // FIX: only emit edges to valid PascalCase type references
                    if !is_valid_type_ref(target_type) {
                        continue;
                    }
                    let edge = (td.name.clone(), target_type.clone());
                    if !edges.contains(&edge) {
                        edges.push(edge);
                    }
                }
            }
        }
    }

    for (from, to) in edges {
        out.push_str(&format!(
            "    {} --> {} : uses\n",
            sanitize_id(&from),
            sanitize_id(&to)
        ));
    }
}

/// Returns true only if `name` looks like a real type reference (PascalCase, no
/// special characters). This filters out local variables, field accesses, ranges,
/// string literals, etc. that the parser may have captured as call targets.
fn is_valid_type_ref(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = match name.chars().next() {
        Some(c) => c,
        None => return false,
    };
    // Type names in Rust are PascalCase (start with uppercase)
    if !first.is_ascii_uppercase() {
        return false;
    }
    // Must not contain characters typical of expressions, not type names
    !name.contains(' ')
        && !name.contains('(')
        && !name.contains(')')
        && !name.contains('.')
        && !name.contains('"')
        && !name.contains('\'')
        && !name.contains('{')
        && !name.contains('[')
}

/// Sanitize a name for use as a Mermaid node ID (no special chars).
fn sanitize_id(name: &str) -> String {
    name.replace("::", "_")
        .replace(['<', '>', ',', ' ', '\'', '(', ')', '.', '[', ']', '{', '}'], "_")
}

/// Sanitize a type string for Mermaid display:
/// 1. Strip Rust lifetime annotations (`'_`, `'a`, `'static`, etc.)
/// 2. Convert generic angle brackets `<T>` → `~T~` (Mermaid syntax)
/// 3. Clean up empty generic brackets left after stripping.
fn sanitize_mermaid(text: &str) -> String {
    let text = strip_lifetimes(text);
    convert_generics(&text)
}

/// Remove Rust lifetime annotations (`'a`, `'_`, `'static`, etc.) from a type string.
fn strip_lifetimes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut result = String::with_capacity(n);
    let mut i = 0;

    while i < n {
        if chars[i] == '\'' {
            // A Rust lifetime: `'` followed by a word character
            let next = i + 1;
            if next < n && (chars[next].is_alphanumeric() || chars[next] == '_') {
                // Skip the lifetime identifier
                i = next;
                while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                // If the lifetime is followed by `, ` (first arg in generics), eat the separator
                if i < n && chars[i] == ',' {
                    let peek = i + 1;
                    if peek < n && chars[peek] == ' ' {
                        i += 2; // skip ", "
                    } else {
                        i += 1; // skip ","
                    }
                }
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    // Clean up empty angle brackets left after stripping lifetimes: "<>" → ""
    result.replace("<>", "")
}

/// Convert `<T>` → `~T~` for Mermaid generic syntax (only outermost brackets).
fn convert_generics(text: &str) -> String {
    let mut result = String::new();
    let mut depth = 0usize;

    for ch in text.chars() {
        match ch {
            '<' => {
                if depth == 0 {
                    result.push('~');
                } else {
                    result.push(ch);
                }
                depth += 1;
            }
            '>' => {
                if depth > 0 {
                    depth -= 1;
                }
                if depth == 0 {
                    result.push('~');
                } else {
                    result.push(ch);
                }
            }
            _ => result.push(ch),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_lifetimes() {
        assert_eq!(strip_lifetimes("Formatter<'_>"), "Formatter");
        assert_eq!(strip_lifetimes("&'a str"), "& str");
        assert_eq!(strip_lifetimes("Span<'static>"), "Span");
        assert_eq!(strip_lifetimes("Node<'a, T>"), "Node<T>");
        assert_eq!(strip_lifetimes("Vec<String>"), "Vec<String>");
    }

    #[test]
    fn test_sanitize_mermaid() {
        assert_eq!(sanitize_mermaid("Vec<Token>"), "Vec~Token~");
        assert_eq!(sanitize_mermaid("Result<AST>"), "Result~AST~");
        assert_eq!(sanitize_mermaid("String"), "String");
        assert_eq!(
            sanitize_mermaid("HashMap<String, Vec<i32>>"),
            "HashMap~String, Vec<i32>~"
        );
        // Lifetime stripping
        assert_eq!(sanitize_mermaid("Formatter<'_>"), "Formatter");
        assert_eq!(sanitize_mermaid("Span<'static>"), "Span");
        assert_eq!(sanitize_mermaid("Node<'a, T>"), "Node~T~");
    }

    #[test]
    fn test_is_valid_type_ref() {
        assert!(is_valid_type_ref("Parser"));
        assert!(is_valid_type_ref("MachineRenderer"));
        assert!(is_valid_type_ref("Vec"));
        assert!(!is_valid_type_ref("self.parser")); // field access
        assert!(!is_valid_type_ref("(0..idx)"));    // range
        assert!(!is_valid_type_ref("\"string\"")); // string literal
        assert!(!is_valid_type_ref("path"));        // lowercase var
        assert!(!is_valid_type_ref(""));
    }

    #[test]
    fn test_mermaid_output() {
        let project = Project {
            modules: vec![Module {
                path: "parser".to_string(),
                language: Language::Rust,
                types: vec![TypeDef {
                    name: "Parser".to_string(),
                    kind: TypeKind::Struct,
                    visibility: Visibility::Public,
                    fields: vec![Field {
                        name: "source".to_string(),
                        type_name: "String".to_string(),
                        visibility: Visibility::Private,
                    }],
                    methods: vec![Method {
                        name: "new".to_string(),
                        params: vec![Param {
                            name: "source".to_string(),
                            type_name: "String".to_string(),
                        }],
                        return_type: Some("Self".to_string()),
                        visibility: Visibility::Public,
                        calls: Vec::new(),
                        annotations: Vec::new(),
                        is_static: true,
                    }],
                    relations: vec![TypeRelation {
                        kind: RelationKind::ImplTrait,
                        target: "Display".to_string(),
                    }],
                    annotations: Vec::new(),
                    type_params: Vec::new(),
                    enum_variants: Vec::new(),
                }],
                functions: Vec::new(),
            }],
        };

        let renderer = MermaidRenderer;
        let output = renderer.render(&project);

        assert!(output.contains("classDiagram"));
        assert!(output.contains("class Parser"));
        assert!(output.contains("-String source"));
        assert!(output.contains("+new(String source) Self$"));
        assert!(output.contains("Parser ..|> Display : implements"));
    }

    #[test]
    fn test_skip_empty_modules() {
        let project = Project {
            modules: vec![
                Module {
                    path: "empty".to_string(),
                    language: Language::Rust,
                    types: vec![],
                    functions: vec![],
                },
                Module {
                    path: "real".to_string(),
                    language: Language::Rust,
                    types: vec![TypeDef {
                        name: "Foo".to_string(),
                        kind: TypeKind::Struct,
                        visibility: Visibility::Public,
                        fields: vec![],
                        methods: vec![],
                        relations: vec![],
                        annotations: vec![],
                        type_params: vec![],
                        enum_variants: vec![],
                    }],
                    functions: vec![],
                },
            ],
        };
        let output = MermaidRenderer.render(&project);
        assert!(!output.contains("## Module: empty"));
        assert!(output.contains("## Module: real"));
    }

    #[test]
    fn test_merge_duplicate_paths() {
        let project = Project {
            modules: vec![
                Module {
                    path: "crate".to_string(),
                    language: Language::Rust,
                    types: vec![],
                    functions: vec![Function {
                        name: "scan".to_string(),
                        params: vec![],
                        return_type: None,
                        visibility: Visibility::Public,
                        calls: vec![],
                    }],
                },
                Module {
                    path: "crate".to_string(),
                    language: Language::Rust,
                    types: vec![TypeDef {
                        name: "Config".to_string(),
                        kind: TypeKind::Struct,
                        visibility: Visibility::Public,
                        fields: vec![],
                        methods: vec![],
                        relations: vec![],
                        annotations: vec![],
                        type_params: vec![],
                        enum_variants: vec![],
                    }],
                    functions: vec![],
                },
            ],
        };
        let output = MermaidRenderer.render(&project);
        // Should appear only once
        assert_eq!(output.matches("## Module: crate").count(), 1);
        // Both contents should be present
        assert!(output.contains("+scan()"));
        assert!(output.contains("class Config"));
    }
}
