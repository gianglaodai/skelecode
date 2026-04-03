use std::path::Path;
use tree_sitter::{Node, Parser};

use super::LanguageParser;
use crate::ir::*;

pub struct RustParser {
    parser: std::cell::RefCell<Parser>,
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RustParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Error loading Rust grammar");
        Self {
            parser: std::cell::RefCell::new(parser),
        }
    }
}

impl LanguageParser for RustParser {
    fn language(&self) -> Language {
        Language::Rust
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "rs")
    }

    fn parse_file(&self, path: &Path, source: &str) -> Option<Module> {
        let tree = self.parser.borrow_mut().parse(source, None)?;
        let root = tree.root_node();

        let module_path = module_path_from_file(path);
        let mut types = Vec::new();
        let mut functions = Vec::new();

        // Collect all impl blocks first, then attach methods to types
        let mut impl_blocks: Vec<ImplBlock> = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "struct_item" => {
                    if let Some(td) = parse_struct(child, source) {
                        types.push(td);
                    }
                }
                "enum_item" => {
                    if let Some(td) = parse_enum(child, source) {
                        types.push(td);
                    }
                }
                "trait_item" => {
                    if let Some(td) = parse_trait(child, source) {
                        types.push(td);
                    }
                }
                "impl_item" => {
                    if let Some(ib) = parse_impl_block(child, source) {
                        impl_blocks.push(ib);
                    }
                }
                "function_item" => {
                    if let Some(f) = parse_free_function(child, source) {
                        functions.push(f);
                    }
                }
                _ => {}
            }
        }

        // Attach impl block methods and relations to their types
        for ib in impl_blocks {
            if let Some(td) = types.iter_mut().find(|t| t.name == ib.type_name) {
                td.methods.extend(ib.methods);
                if let Some(rel) = ib.trait_relation {
                    td.relations.push(rel);
                }
            } else {
                // Type not found in this file — create a minimal TypeDef
                let mut td = TypeDef {
                    name: ib.type_name,
                    kind: TypeKind::Struct,
                    visibility: Visibility::Private,
                    fields: Vec::new(),
                    methods: ib.methods,
                    relations: Vec::new(),
                    annotations: Vec::new(),
                    type_params: Vec::new(),
                    enum_variants: Vec::new(),
                };
                if let Some(rel) = ib.trait_relation {
                    td.relations.push(rel);
                }
                types.push(td);
            }
        }

        Some(Module {
            path: module_path,
            language: Language::Rust,
            types,
            functions,
        })
    }
}

struct ImplBlock {
    type_name: String,
    trait_relation: Option<TypeRelation>,
    methods: Vec<Method>,
}

fn module_path_from_file(path: &Path) -> String {
    let p = path.with_extension("");
    let components: Vec<&str> = p
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Try to find "src" and take everything after it
    if let Some(src_idx) = components.iter().position(|&c| c == "src") {
        let after_src: Vec<&str> = components[src_idx + 1..].to_vec();
        if after_src.is_empty() {
            return "crate".to_string();
        }
        let path_str = after_src.join("::");
        // lib.rs and main.rs map to crate root
        if path_str == "lib" || path_str == "main" {
            return "crate".to_string();
        }
        // mod.rs maps to parent module
        if let Some(stripped) = path_str.strip_suffix("::mod") {
            return stripped.to_string();
        }
        return path_str;
    }

    // Fallback: use file stem
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn extract_visibility(node: Node, source: &str) -> Visibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = node_text(child, source);
            return match text {
                "pub" => Visibility::Public,
                "pub(crate)" => Visibility::Crate,
                _ if text.starts_with("pub(") => Visibility::Crate,
                _ => Visibility::Public,
            };
        }
    }
    Visibility::Private
}

fn extract_type_params(node: Node, source: &str) -> Vec<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_parameters" {
            let mut params = Vec::new();
            collect_type_params(child, source, &mut params);
            return params;
        }
    }
    Vec::new()
}

fn collect_type_params(node: Node, source: &str, params: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                params.push(node_text(child, source).to_string());
            }
            "lifetime" => {
                params.push(node_text(child, source).to_string());
            }
            "constrained_type_parameter" => {
                // Take just the name (first type_identifier child)
                if let Some(name_node) = child.child_by_field_name("left") {
                    params.push(node_text(name_node, source).to_string());
                } else if let Some(first) = child.child(0) {
                    params.push(node_text(first, source).to_string());
                }
            }
            "type_parameter" => {
                // Recurse into type_parameter wrapper
                collect_type_params(child, source, params);
            }
            _ => {}
        }
    }
}

fn parse_struct(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let visibility = extract_visibility(node, source);
    let type_params = extract_type_params(node, source);

    let mut fields = Vec::new();

    // Look for field_declaration_list (struct body)
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "field_declaration"
                && let Some(field) = parse_field(child, source)
            {
                fields.push(field);
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Struct,
        visibility,
        fields,
        methods: Vec::new(),
        relations: Vec::new(),
        annotations: Vec::new(),
        type_params,
        enum_variants: Vec::new(),
    })
}

fn parse_field(node: Node, source: &str) -> Option<Field> {
    let name = node.child_by_field_name("name")?;
    let type_node = node.child_by_field_name("type")?;
    let visibility = extract_visibility(node, source);

    Some(Field {
        name: node_text(name, source).to_string(),
        type_name: node_text(type_node, source).to_string(),
        visibility,
    })
}

fn parse_enum(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let visibility = extract_visibility(node, source);
    let type_params = extract_type_params(node, source);

    let mut variants = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enum_variant"
                && let Some(vname) = child.child_by_field_name("name")
            {
                let variant_text = node_text(vname, source).to_string();
                // Check for tuple or struct body
                let mut has_body = false;
                let mut inner_cursor = child.walk();
                for vc in child.children(&mut inner_cursor) {
                    if vc.kind() == "field_declaration_list"
                        || vc.kind() == "ordered_field_declaration_list"
                    {
                        let body_text = node_text(vc, source);
                        variants.push(format!("{}{}", variant_text, body_text));
                        has_body = true;
                        break;
                    }
                }
                if !has_body {
                    variants.push(variant_text);
                }
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Enum,
        visibility,
        fields: Vec::new(),
        methods: Vec::new(),
        relations: Vec::new(),
        annotations: Vec::new(),
        type_params,
        enum_variants: variants,
    })
}

fn parse_trait(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let visibility = extract_visibility(node, source);
    let type_params = extract_type_params(node, source);

    let mut methods = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if (child.kind() == "function_signature_item" || child.kind() == "function_item")
                && let Some(m) = parse_method(child, source, false)
            {
                methods.push(m);
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Trait,
        visibility,
        fields: Vec::new(),
        methods,
        relations: Vec::new(),
        annotations: Vec::new(),
        type_params,
        enum_variants: Vec::new(),
    })
}

fn parse_impl_block(node: Node, source: &str) -> Option<ImplBlock> {
    let type_node = node.child_by_field_name("type")?;
    let type_name = node_text(type_node, source).to_string();

    // Check for trait impl: `impl Trait for Type`
    let trait_relation = node
        .child_by_field_name("trait")
        .map(|trait_node| TypeRelation {
            kind: RelationKind::ImplTrait,
            target: node_text(trait_node, source).to_string(),
        });

    let mut methods = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_item"
                && let Some(m) = parse_method(child, source, true)
            {
                methods.push(m);
            }
        }
    }

    Some(ImplBlock {
        type_name,
        trait_relation,
        methods,
    })
}

fn parse_method(node: Node, source: &str, _in_impl: bool) -> Option<Method> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let visibility = extract_visibility(node, source);

    let (params, is_static) = parse_parameters(node, source);
    let return_type = parse_return_type(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Method {
        name: name_str,
        params,
        return_type,
        visibility,
        calls,
        annotations: Vec::new(),
        is_static,
    })
}

fn parse_free_function(node: Node, source: &str) -> Option<Function> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let visibility = extract_visibility(node, source);

    let (params, _) = parse_parameters(node, source);
    let return_type = parse_return_type(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Function {
        name: name_str,
        params,
        return_type,
        visibility,
        calls,
    })
}

fn parse_parameters(node: Node, source: &str) -> (Vec<Param>, bool) {
    let mut params = Vec::new();
    let mut is_static = true;

    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "self_parameter" => {
                    is_static = false;
                    // Don't add self to param list
                }
                "parameter" => {
                    let pattern = child.child_by_field_name("pattern");
                    let type_node = child.child_by_field_name("type");
                    if let Some(type_n) = type_node {
                        let pname = pattern
                            .map(|p| node_text(p, source).to_string())
                            .unwrap_or_default();
                        let ptype = node_text(type_n, source).to_string();
                        params.push(Param {
                            name: pname,
                            type_name: ptype,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    (params, is_static)
}

fn parse_return_type(node: Node, source: &str) -> Option<String> {
    let ret = node.child_by_field_name("return_type")?;
    // The "return_type" field already points to the actual type node.
    let text = node_text(ret, source).trim();
    let clean_text = text.trim_start_matches("->").trim();
    Some(clean_text.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn extract_calls(node: Node, source: &str) -> Vec<CallRef> {
    let mut calls = Vec::new();
    collect_calls(node, source, &mut calls);

    // Deduplicate
    calls.sort_by(|a, b| {
        let a_str = format!("{}", a);
        let b_str = format!("{}", b);
        a_str.cmp(&b_str)
    });
    calls.dedup_by(|a, b| format!("{}", a) == format!("{}", b));

    calls
}

/// Resolve a receiver expression to a simple name.
/// For `self` → "self", for `self.field` → "self.field",
/// for identifiers → the identifier text,
/// for complex expressions (chains) → just the root identifier or "self".
fn resolve_receiver(node: Node, source: &str) -> String {
    let raw = match node.kind() {
        "self" | "identifier" => node_text(node, source).to_string(),
        "field_expression" => {
            // e.g., self.repo or self.parser
            if let (Some(obj), Some(field)) = (
                node.child_by_field_name("value"),
                node.child_by_field_name("field"),
            ) {
                let obj_text = node_text(obj, source);
                if obj_text == "self" {
                    // self.field → use field name as receiver
                    return format!("self.{}", node_text(field, source).trim());
                }
                // obj.field — just return the immediate object name
                return resolve_receiver(obj, source);
            }
            node_text(node, source).to_string()
        }
        // For call expressions used as receiver (method chaining), skip them
        "call_expression" => {
            if let Some(func) = node.child_by_field_name("function") {
                return resolve_receiver(func, source);
            }
            "?".to_string()
        }
        _ => node_text(node, source).to_string(),
    };
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_calls(node: Node, source: &str, calls: &mut Vec<CallRef>) {
    match node.kind() {
        "call_expression" => {
            if let Some(func) = node.child_by_field_name("function") {
                match func.kind() {
                    // Type::method() or module::function()
                    "scoped_identifier" => {
                        let text = node_text(func, source);
                        if let Some((scope, method)) = text.rsplit_once("::") {
                            calls.push(CallRef {
                                target_type: Some(scope.split_whitespace().collect::<Vec<_>>().join(" ")),
                                target_method: method.split_whitespace().collect::<Vec<_>>().join(" "),
                            });
                        }
                    }
                    // plain function call: foo()
                    "identifier" => {
                        calls.push(CallRef {
                            target_type: None,
                            target_method: node_text(func, source).split_whitespace().collect::<Vec<_>>().join(" "),
                        });
                    }
                    // field_expression: self.method() or obj.method()
                    "field_expression" => {
                        if let (Some(obj), Some(field)) = (
                            func.child_by_field_name("value"),
                            func.child_by_field_name("field"),
                        ) {
                            let method_text = node_text(field, source).split_whitespace().collect::<Vec<_>>().join(" ");
                            // Resolve the receiver to a simple name
                            let receiver = resolve_receiver(obj, source);
                            calls.push(CallRef {
                                target_type: Some(receiver),
                                target_method: method_text,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        // Macro invocations like println!, vec!, etc — skip these
        "macro_invocation" => return,
        _ => {}
    }

    // Recurse into children (but skip into call_expression we already handled)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, source, calls);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Module {
        let parser = RustParser::new();
        parser
            .parse_file(Path::new("src/test.rs"), source)
            .expect("parse failed")
    }

    #[test]
    fn test_simple_struct() {
        let m = parse(
            r#"
pub struct Foo {
    pub name: String,
    count: usize,
}
"#,
        );
        assert_eq!(m.types.len(), 1);
        let t = &m.types[0];
        assert_eq!(t.name, "Foo");
        assert_eq!(t.kind, TypeKind::Struct);
        assert_eq!(t.visibility, Visibility::Public);
        assert_eq!(t.fields.len(), 2);
        assert_eq!(t.fields[0].name, "name");
        assert_eq!(t.fields[0].type_name, "String");
        assert_eq!(t.fields[0].visibility, Visibility::Public);
        assert_eq!(t.fields[1].name, "count");
        assert_eq!(t.fields[1].visibility, Visibility::Private);
    }

    #[test]
    fn test_enum_with_variants() {
        let m = parse(
            r#"
pub enum Color {
    Red,
    Green,
    Blue,
    Custom(u8, u8, u8),
}
"#,
        );
        assert_eq!(m.types.len(), 1);
        let t = &m.types[0];
        assert_eq!(t.name, "Color");
        assert_eq!(t.kind, TypeKind::Enum);
        assert_eq!(t.enum_variants.len(), 4);
        assert_eq!(t.enum_variants[0], "Red");
        assert_eq!(t.enum_variants[3], "Custom(u8, u8, u8)");
    }

    #[test]
    fn test_trait_definition() {
        let m = parse(
            r#"
pub trait Drawable {
    fn draw(&self);
    fn resize(&mut self, width: u32, height: u32) -> bool;
}
"#,
        );
        assert_eq!(m.types.len(), 1);
        let t = &m.types[0];
        assert_eq!(t.name, "Drawable");
        assert_eq!(t.kind, TypeKind::Trait);
        assert_eq!(t.methods.len(), 2);
        assert_eq!(t.methods[0].name, "draw");
        assert!(!t.methods[0].is_static);
        assert_eq!(t.methods[1].name, "resize");
        assert_eq!(t.methods[1].params.len(), 2);
    }

    #[test]
    fn test_impl_block_attaches_to_struct() {
        let m = parse(
            r#"
pub struct Parser {
    source: String,
    pos: usize,
}

impl Parser {
    pub fn new(source: String) -> Self {
        Parser { source, pos: 0 }
    }

    pub fn parse(&self) -> Vec<Token> {
        self.tokenize()
    }

    fn tokenize(&self) -> Vec<Token> {
        Vec::new()
    }
}
"#,
        );
        assert_eq!(m.types.len(), 1);
        let t = &m.types[0];
        assert_eq!(t.name, "Parser");
        assert_eq!(t.methods.len(), 3);
        assert!(t.methods[0].is_static); // new() has no self
        assert!(!t.methods[1].is_static); // parse() has &self
    }

    #[test]
    fn test_trait_impl() {
        let m = parse(
            r#"
pub struct MyType;

impl Display for MyType {
    fn fmt(&self, f: &mut Formatter) -> Result {
        Ok(())
    }
}
"#,
        );
        assert_eq!(m.types.len(), 1);
        let t = &m.types[0];
        assert_eq!(t.relations.len(), 1);
        assert_eq!(t.relations[0].target, "Display");
        assert_eq!(t.relations[0].kind, RelationKind::ImplTrait);
    }

    #[test]
    fn test_call_extraction() {
        let m = parse(
            r#"
pub struct Service {
    repo: Repository,
}

impl Service {
    pub fn process(&self) {
        let data = self.fetch();
        let result = Parser::parse(data);
        helper(result);
        self.repo.save(result);
    }
}
"#,
        );
        let t = &m.types[0];
        let method = &t.methods[0];
        assert_eq!(method.name, "process");

        let call_strs: Vec<String> = method.calls.iter().map(|c| format!("{}", c)).collect();
        assert!(call_strs.contains(&"self::fetch".to_string()));
        assert!(call_strs.contains(&"Parser::parse".to_string()));
        assert!(call_strs.contains(&"helper".to_string()));
    }

    #[test]
    fn test_free_functions() {
        let m = parse(
            r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn helper() {
    add(1, 2);
}
"#,
        );
        assert_eq!(m.functions.len(), 2);
        assert_eq!(m.functions[0].name, "add");
        assert_eq!(m.functions[0].visibility, Visibility::Public);
        assert_eq!(m.functions[0].params.len(), 2);
        assert_eq!(m.functions[0].return_type.as_deref(), Some("i32"));
        assert_eq!(m.functions[1].name, "helper");
        assert_eq!(m.functions[1].calls.len(), 1);
    }

    #[test]
    fn test_generics() {
        let m = parse(
            r#"
pub struct Container<T, U> {
    value: T,
    extra: U,
}
"#,
        );
        let t = &m.types[0];
        assert_eq!(t.type_params, vec!["T", "U"]);
    }

    #[test]
    fn test_module_path() {
        assert_eq!(module_path_from_file(Path::new("src/main.rs")), "crate");
        assert_eq!(module_path_from_file(Path::new("src/lib.rs")), "crate");
        assert_eq!(
            module_path_from_file(Path::new("src/parser/mod.rs")),
            "parser"
        );
        assert_eq!(
            module_path_from_file(Path::new("src/parser/rust.rs")),
            "parser::rust"
        );
    }
}
