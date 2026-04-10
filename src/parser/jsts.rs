use std::path::Path;
use tree_sitter::{Node, Parser};

use super::LanguageParser;
use crate::ir::*;

/// Unified parser for JavaScript (.js, .jsx) and TypeScript (.ts, .tsx) files.
/// Uses tree-sitter-javascript for JS/JSX and tree-sitter-typescript for TS/TSX.
pub struct JsTsParser {
    js_parser: std::cell::RefCell<Parser>,
    ts_parser: std::cell::RefCell<Parser>,
    tsx_parser: std::cell::RefCell<Parser>,
}

impl Default for JsTsParser {
    fn default() -> Self {
        Self::new()
    }
}

impl JsTsParser {
    pub fn new() -> Self {
        let mut js_parser = Parser::new();
        js_parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .expect("Error loading JavaScript grammar");

        let mut ts_parser = Parser::new();
        ts_parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .expect("Error loading TypeScript grammar");

        let mut tsx_parser = Parser::new();
        tsx_parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
            .expect("Error loading TSX grammar");

        Self {
            js_parser: std::cell::RefCell::new(js_parser),
            ts_parser: std::cell::RefCell::new(ts_parser),
            tsx_parser: std::cell::RefCell::new(tsx_parser),
        }
    }

    fn parser_for_ext(&self, ext: &str) -> &std::cell::RefCell<Parser> {
        match ext {
            "ts" => &self.ts_parser,
            "tsx" | "jsx" => &self.tsx_parser,
            _ => &self.js_parser,
        }
    }
}

impl LanguageParser for JsTsParser {
    fn language(&self) -> Language {
        Language::JavaScript
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e, "js" | "jsx" | "ts" | "tsx"))
    }

    fn parse_file(&self, path: &Path, source: &str) -> Option<Module> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("js");
        let is_typescript = matches!(ext, "ts" | "tsx");

        let parser_cell = self.parser_for_ext(ext);
        let tree = parser_cell.borrow_mut().parse(source, None)?;
        let root = tree.root_node();

        let mut types = Vec::new();
        let mut functions = Vec::new();
        let mut imports = Vec::new();

        parse_children(root, source, is_typescript, &mut types, &mut functions, &mut imports);

        let module_path = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Some(Module {
            path: module_path,
            language: Language::JavaScript,
            types,
            functions,
            imports,
        })
    }
}

fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn parse_children(
    node: Node,
    source: &str,
    is_typescript: bool,
    types: &mut Vec<TypeDef>,
    functions: &mut Vec<Function>,
    imports: &mut Vec<ImportedName>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                imports.extend(parse_js_import(child, source));
            }
            // class Foo { ... }
            "class_declaration" => {
                if let Some(td) = parse_class(child, source, is_typescript) {
                    types.push(td);
                }
            }
            // export default class Foo { ... }
            "export_statement" => {
                parse_export(child, source, is_typescript, types, functions, imports);
            }
            // function foo() { ... }
            "function_declaration" => {
                if let Some(f) = parse_function(child, source) {
                    functions.push(f);
                }
            }
            // const foo = () => { ... } or const foo = function() { ... }
            "lexical_declaration" | "variable_declaration" => {
                functions.extend(parse_variable_functions(child, source));
            }
            // TypeScript-specific: interface, enum, type alias
            "interface_declaration" if is_typescript => {
                if let Some(td) = parse_ts_interface(child, source) {
                    types.push(td);
                }
            }
            "enum_declaration" if is_typescript => {
                if let Some(td) = parse_ts_enum(child, source) {
                    types.push(td);
                }
            }
            "abstract_class_declaration" if is_typescript => {
                if let Some(td) = parse_class(child, source, is_typescript) {
                    types.push(td);
                }
            }
            _ => {}
        }
    }
}

fn parse_export(
    node: Node,
    source: &str,
    is_typescript: bool,
    types: &mut Vec<TypeDef>,
    functions: &mut Vec<Function>,
    _imports: &mut Vec<ImportedName>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(mut td) = parse_class(child, source, is_typescript) {
                    td.visibility = Visibility::Public;
                    types.push(td);
                }
            }
            "function_declaration" => {
                if let Some(mut f) = parse_function(child, source) {
                    f.visibility = Visibility::Public;
                    functions.push(f);
                }
            }
            "interface_declaration" if is_typescript => {
                if let Some(mut td) = parse_ts_interface(child, source) {
                    td.visibility = Visibility::Public;
                    types.push(td);
                }
            }
            "enum_declaration" if is_typescript => {
                if let Some(mut td) = parse_ts_enum(child, source) {
                    td.visibility = Visibility::Public;
                    types.push(td);
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                for mut f in parse_variable_functions(child, source) {
                    f.visibility = Visibility::Public;
                    functions.push(f);
                }
            }
            _ => {}
        }
    }
}

// ── Import parsing ─────────────────────────────────────────────────────────────

/// Parse `import { A, B as C } from './path/Module'` and variants.
/// The module stem is used as the module path component.
fn parse_js_import(node: Node, source: &str) -> Vec<ImportedName> {
    let mut module_stem = String::new();
    let mut names: Vec<(String, String)> = Vec::new(); // (alias, original_name)

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_clause" => collect_import_clause(child, source, &mut names),
            // The source string: './path/to/Module' or 'react'
            "string" => {
                let raw = node_text(child, source);
                let s = raw.trim_matches(|c| c == '\'' || c == '"');
                let stem = s.rsplit('/').next().unwrap_or(s);
                let stem = stem.split('.').next().unwrap_or(stem);
                module_stem = stem.to_string();
            }
            _ => {}
        }
    }

    if module_stem.is_empty() || names.is_empty() {
        return Vec::new();
    }

    names
        .into_iter()
        .filter(|(_, orig)| orig.starts_with(|c: char| c.is_uppercase()))
        .map(|(alias, orig)| ImportedName {
            alias,
            qualified: format!("{}::{}", module_stem, orig),
        })
        .collect()
}

fn collect_import_clause(node: Node, source: &str, names: &mut Vec<(String, String)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // Default import: `import Foo from ...`
            "identifier" => {
                let name = node_text(child, source).to_string();
                names.push((name.clone(), name));
            }
            // Named imports: `{ A, B as C }`
            "named_imports" => collect_named_imports(child, source, names),
            // Namespace import `* as X` — skip
            _ => {}
        }
    }
}

fn collect_named_imports(node: Node, source: &str, names: &mut Vec<(String, String)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "import_specifier" {
            let orig = child
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let alias = child
                .child_by_field_name("alias")
                .map(|a| node_text(a, source).to_string())
                .unwrap_or_else(|| orig.clone());
            if !orig.is_empty() {
                names.push((alias, orig));
            }
        }
    }
}

// ── Class parsing ──────────────────────────────────────────────────────────────

fn parse_class(node: Node, source: &str, is_typescript: bool) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut relations = Vec::new();
    let type_params = extract_type_params(node, source);

    // Heritage: extends / implements
    // Both JS and TS use `class_heritage` as a child of class_declaration.
    // Inside class_heritage:
    //   JS: `extends` keyword + identifier (superclass)
    //   TS: `extends_clause` and/or `implements_clause`
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_heritage" => {
                parse_class_heritage(child, source, &mut relations);
            }
            "extends_clause" => {
                parse_extends_clause(child, source, &mut relations);
            }
            "implements_clause" => {
                parse_implements_clause(child, source, &mut relations);
            }
            _ => {}
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        parse_class_body(body, source, is_typescript, &mut fields, &mut methods);
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Class,
        visibility: Visibility::Internal,
        fields,
        methods,
        relations,
        annotations: extract_decorators(node, source),
        type_params,
        enum_variants: Vec::new(),
    })
}

fn parse_class_heritage(node: Node, source: &str, relations: &mut Vec<TypeRelation>) {
    // class_heritage children:
    //   JS:  extends keyword + identifier
    //   TS:  extends_clause { extends keyword + type } and/or implements_clause
    let mut saw_extends = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "extends" => {
                saw_extends = true;
            }
            "extends_clause" => {
                parse_extends_clause(child, source, relations);
            }
            "implements_clause" => {
                parse_implements_clause(child, source, relations);
            }
            _ if saw_extends && child.is_named() => {
                // JS: the identifier right after "extends" keyword
                relations.push(TypeRelation {
                    kind: RelationKind::Extends,
                    target: node_text(child, source).to_string(),
                });
                saw_extends = false;
            }
            _ => {}
        }
    }
}

fn parse_extends_clause(node: Node, source: &str, relations: &mut Vec<TypeRelation>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Skip the "extends" keyword itself
        if child.kind() == "extends" {
            continue;
        }
        if child.is_named() {
            relations.push(TypeRelation {
                kind: RelationKind::Extends,
                target: node_text(child, source).to_string(),
            });
        }
    }
}

fn parse_implements_clause(node: Node, source: &str, relations: &mut Vec<TypeRelation>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "implements" {
            continue;
        }
        if child.is_named() {
            relations.push(TypeRelation {
                kind: RelationKind::Implements,
                target: node_text(child, source).to_string(),
            });
        }
    }
}

fn parse_class_body(
    body: Node,
    source: &str,
    is_typescript: bool,
    fields: &mut Vec<Field>,
    methods: &mut Vec<Method>,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "method_definition" => {
                if let Some(m) = parse_method(child, source) {
                    methods.push(m);
                }
            }
            // TS: public/private/protected fields and methods
            "public_field_definition" | "field_definition" => {
                if let Some(f) = parse_field(child, source, is_typescript) {
                    // Check if value is function — then it's a method-like field
                    fields.push(f);
                }
            }
            // TS abstract method
            "abstract_method_signature" if is_typescript => {
                if let Some(m) = parse_abstract_method(child, source) {
                    methods.push(m);
                }
            }
            _ => {}
        }
    }
}

fn parse_method(node: Node, source: &str) -> Option<Method> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();

    let vis = extract_accessibility(node, source);
    let is_static = has_child_text(node, source, "static");
    let params = extract_params(node, source);
    let return_type = extract_return_type(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Method {
        name: name_str,
        params,
        return_type,
        visibility: vis,
        calls,
        callers: Vec::new(),
        annotations: extract_decorators(node, source),
        is_static,
    })
}

fn parse_abstract_method(node: Node, source: &str) -> Option<Method> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();

    let vis = extract_accessibility(node, source);
    let params = extract_params(node, source);
    let return_type = extract_return_type(node, source);

    Some(Method {
        name: name_str,
        params,
        return_type,
        visibility: vis,
        calls: Vec::new(),
        callers: Vec::new(),
        annotations: Vec::new(),
        is_static: false,
    })
}

fn parse_field(node: Node, source: &str, is_typescript: bool) -> Option<Field> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();

    let type_name = if is_typescript {
        extract_type_annotation(node, source).unwrap_or_default()
    } else {
        String::new()
    };

    let vis = extract_accessibility(node, source);

    Some(Field {
        name: name_str,
        type_name,
        visibility: vis,
    })
}

// ── Function parsing ───────────────────────────────────────────────────────────

fn parse_function(node: Node, source: &str) -> Option<Function> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();

    let params = extract_params(node, source);
    let return_type = extract_return_type(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Function {
        name: name_str,
        params,
        return_type,
        visibility: Visibility::Internal,
        calls,
        callers: Vec::new(),
    })
}

/// Parse const/let/var declarations that assign arrow functions or function expressions.
/// e.g. `const greet = (name: string): void => { ... }`
fn parse_variable_functions(node: Node, source: &str) -> Vec<Function> {
    let mut functions = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = match child.child_by_field_name("name") {
                Some(n) if n.kind() == "identifier" => node_text(n, source).to_string(),
                _ => continue,
            };

            let value = match child.child_by_field_name("value") {
                Some(v) => v,
                None => continue,
            };

            let is_func = matches!(
                value.kind(),
                "arrow_function" | "function_expression" | "function"
            );

            if !is_func {
                continue;
            }

            let params = extract_params(value, source);
            let return_type = extract_return_type(value, source);

            let calls = if let Some(body) = value.child_by_field_name("body") {
                extract_calls(body, source)
            } else {
                Vec::new()
            };

            functions.push(Function {
                name,
                params,
                return_type,
                visibility: Visibility::Internal,
                calls,
                callers: Vec::new(),
            });
        }
    }
    functions
}

// ── TypeScript interface ───────────────────────────────────────────────────────

fn parse_ts_interface(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();

    let type_params = extract_type_params(node, source);
    let mut relations = Vec::new();
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    // extends clause
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "extends_type_clause" || child.kind() == "extends_clause" {
            parse_extends_clause(child, source, &mut relations);
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for child in body.children(&mut body_cursor) {
            match child.kind() {
                "property_signature" => {
                    if let Some(prop_name) = child.child_by_field_name("name") {
                        let type_name =
                            extract_type_annotation(child, source).unwrap_or_default();
                        fields.push(Field {
                            name: node_text(prop_name, source).to_string(),
                            type_name,
                            visibility: Visibility::Public,
                        });
                    }
                }
                "method_signature" => {
                    if let Some(sig_name) = child.child_by_field_name("name") {
                        let params = extract_params(child, source);
                        let return_type = extract_return_type(child, source);
                        methods.push(Method {
                            name: node_text(sig_name, source).to_string(),
                            params,
                            return_type,
                            visibility: Visibility::Public,
                            calls: Vec::new(),
                            callers: Vec::new(),
                            annotations: Vec::new(),
                            is_static: false,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Interface,
        visibility: Visibility::Internal,
        fields,
        methods,
        relations,
        annotations: Vec::new(),
        type_params,
        enum_variants: Vec::new(),
    })
}

// ── TypeScript enum ────────────────────────────────────────────────────────────

fn parse_ts_enum(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();

    let mut variants = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                // tree-sitter-typescript uses property_identifier directly in enum_body
                "property_identifier" | "identifier" => {
                    variants.push(node_text(child, source).to_string());
                }
                // Some versions use enum_member or enum_assignment
                "enum_member" | "enum_assignment" => {
                    if let Some(member_name) = child.child_by_field_name("name") {
                        let variant = node_text(member_name, source).to_string();
                        if let Some(value) = child.child_by_field_name("value") {
                            variants.push(format!("{} = {}", variant, node_text(value, source)));
                        } else {
                            variants.push(variant);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Enum,
        visibility: Visibility::Internal,
        fields: Vec::new(),
        methods: Vec::new(),
        relations: Vec::new(),
        annotations: Vec::new(),
        type_params: Vec::new(),
        enum_variants: variants,
    })
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn extract_type_params(node: Node, source: &str) -> Vec<String> {
    if let Some(tp) = node.child_by_field_name("type_parameters") {
        let mut params = Vec::new();
        let mut cursor = tp.walk();
        for child in tp.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                params.push(node_text(child, source).to_string());
            }
        }
        return params;
    }
    Vec::new()
}

fn extract_params(node: Node, source: &str) -> Vec<Param> {
    let params_node = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("formal_parameters"));

    let Some(params_node) = params_node else {
        return Vec::new();
    };

    let mut params = Vec::new();
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        match child.kind() {
            "formal_parameter" | "required_parameter" | "optional_parameter" => {
                // TS: pattern (name) + type_annotation
                // JS: just the name/pattern
                let name = child
                    .child_by_field_name("pattern")
                    .or_else(|| child.child_by_field_name("name"))
                    .map(|n| node_text(n, source).to_string())
                    .unwrap_or_default();

                let type_name = extract_type_annotation(child, source).unwrap_or_default();

                params.push(Param { name, type_name });
            }
            "rest_parameter" => {
                let name = child
                    .child_by_field_name("pattern")
                    .or_else(|| child.child_by_field_name("name"))
                    .map(|n| format!("...{}", node_text(n, source)))
                    .unwrap_or_else(|| "...".to_string());

                let type_name = extract_type_annotation(child, source).unwrap_or_default();
                params.push(Param { name, type_name });
            }
            // Simple identifier param (JS)
            "identifier" => {
                params.push(Param {
                    name: node_text(child, source).to_string(),
                    type_name: String::new(),
                });
            }
            _ => {}
        }
    }
    params
}

fn extract_return_type(node: Node, source: &str) -> Option<String> {
    if let Some(ret) = node.child_by_field_name("return_type") {
        // return_type is a type_annotation node like ": string"
        let text = node_text(ret, source).trim().to_string();
        let clean = text.strip_prefix(':').unwrap_or(&text).trim().to_string();
        if !clean.is_empty() {
            return Some(clean);
        }
    }
    None
}

fn extract_type_annotation(node: Node, source: &str) -> Option<String> {
    if let Some(ta) = node.child_by_field_name("type") {
        return Some(node_text(ta, source).to_string());
    }
    // Also check for type_annotation child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_annotation" {
            let text = node_text(child, source).trim().to_string();
            let clean = text.strip_prefix(':').unwrap_or(&text).trim().to_string();
            if !clean.is_empty() {
                return Some(clean);
            }
        }
    }
    None
}

fn extract_accessibility(node: Node, source: &str) -> Visibility {
    // Check for TS accessibility modifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "accessibility_modifier" {
            return match node_text(child, source) {
                "public" => Visibility::Public,
                "private" => Visibility::Private,
                "protected" => Visibility::Protected,
                _ => Visibility::Public,
            };
        }
    }
    Visibility::Public
}

fn extract_decorators(node: Node, source: &str) -> Vec<Annotation> {
    let mut annotations = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "decorator" {
            // decorator text is like @Component({...}) — extract just the name
            let text = node_text(child, source).to_string();
            let name = text
                .strip_prefix('@')
                .unwrap_or(&text)
                .split('(')
                .next()
                .unwrap_or(&text)
                .to_string();
            annotations.push(Annotation { name });
        }
    }
    annotations
}

fn has_child_text(node: Node, source: &str, text: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if node_text(child, source) == text {
            return true;
        }
    }
    false
}

// ── Call extraction ────────────────────────────────────────────────────────────

fn extract_calls(node: Node, source: &str) -> Vec<CallRef> {
    let mut calls = Vec::new();
    collect_calls(node, source, &mut calls);

    calls.sort_by(|a, b| {
        let a_str = format!("{}", a);
        let b_str = format!("{}", b);
        a_str.cmp(&b_str)
    });
    calls.dedup_by(|a, b| format!("{}", a) == format!("{}", b));

    calls
}

fn resolve_receiver(node: Node, source: &str) -> String {
    match node.kind() {
        "this" | "super" | "identifier" => node_text(node, source).to_string(),
        "member_expression" => {
            if let Some(obj) = node.child_by_field_name("object") {
                let obj_text = node_text(obj, source);
                if obj_text == "this"
                    && let Some(prop) = node.child_by_field_name("property")
                {
                    return format!("this.{}", node_text(prop, source));
                }
                return resolve_receiver(obj, source);
            }
            node_text(node, source).to_string()
        }
        "call_expression" => {
            if let Some(func) = node.child_by_field_name("function") {
                return resolve_receiver(func, source);
            }
            "?".to_string()
        }
        _ => node_text(node, source).to_string(),
    }
}

fn collect_calls(node: Node, source: &str, calls: &mut Vec<CallRef>) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            match func.kind() {
                "member_expression" => {
                    // obj.method() or obj.prop.method()
                    if let (Some(obj), Some(prop)) = (
                        func.child_by_field_name("object"),
                        func.child_by_field_name("property"),
                    ) {
                        let receiver = resolve_receiver(obj, source);
                        calls.push(CallRef {
                            target_type: Some(receiver),
                            target_method: node_text(prop, source).to_string(),
                        });
                    }
                }
                "identifier" => {
                    // plain function call: foo()
                    calls.push(CallRef {
                        target_type: None,
                        target_method: node_text(func, source).to_string(),
                    });
                }
                _ => {}
            }
        }
    } else if node.kind() == "new_expression" {
        // new Foo()
        if let Some(constructor) = node.child_by_field_name("constructor") {
            calls.push(CallRef {
                target_type: Some(node_text(constructor, source).to_string()),
                target_method: "<init>".to_string(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, source, calls);
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_parser_basic() {
        let parser = JsTsParser::new();
        let source = r#"
class Animal {
    constructor(name) {
        this.name = name;
    }

    speak() {
        console.log(this.name + " makes a sound.");
    }
}

class Dog extends Animal {
    bark() {
        console.log("Woof!");
    }
}

function createDog(name) {
    return new Dog(name);
}

const greet = (name) => {
    console.log("Hello, " + name);
};
"#;
        let path = Path::new("test.js");
        let module = parser.parse_file(path, source).unwrap();

        assert_eq!(module.language, Language::JavaScript);

        // 2 classes: Animal, Dog
        assert_eq!(module.types.len(), 2);

        let animal = &module.types[0];
        assert_eq!(animal.name, "Animal");
        assert_eq!(animal.kind, TypeKind::Class);
        // constructor + speak
        assert_eq!(animal.methods.len(), 2);

        let dog = &module.types[1];
        assert_eq!(dog.name, "Dog");
        // Dog extends Animal
        assert!(dog.relations.iter().any(|r| r.kind == RelationKind::Extends && r.target.contains("Animal")));

        // 2 functions: createDog, greet
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions[0].name, "createDog");
        assert_eq!(module.functions[1].name, "greet");
    }

    #[test]
    fn test_ts_parser_basic() {
        let parser = JsTsParser::new();
        let source = r#"
interface Serializable {
    serialize(): string;
    deserialize(data: string): void;
}

interface Loggable {
    log(message: string): void;
}

enum Status {
    Active,
    Inactive,
    Pending = "PENDING",
}

class UserService implements Serializable, Loggable {
    private name: string;
    public email: string;

    constructor(name: string, email: string) {
        this.name = name;
        this.email = email;
    }

    serialize(): string {
        return JSON.stringify({ name: this.name, email: this.email });
    }

    deserialize(data: string): void {
        const parsed = JSON.parse(data);
        this.name = parsed.name;
    }

    log(message: string): void {
        console.log(message);
    }

    static create(name: string, email: string): UserService {
        return new UserService(name, email);
    }
}

export function fetchUsers(): Promise<UserService[]> {
    return fetch("/api/users").then(res => res.json());
}

export const processUser = (user: UserService): void => {
    user.serialize();
};
"#;
        let path = Path::new("test.ts");
        let module = parser.parse_file(path, source).unwrap();

        assert_eq!(module.language, Language::JavaScript);

        // interface Serializable, interface Loggable, enum Status, class UserService
        assert_eq!(module.types.len(), 4);

        let serializable = &module.types[0];
        assert_eq!(serializable.name, "Serializable");
        assert_eq!(serializable.kind, TypeKind::Interface);
        assert_eq!(serializable.methods.len(), 2);

        let status = &module.types[2];
        assert_eq!(status.name, "Status");
        assert_eq!(status.kind, TypeKind::Enum);
        assert_eq!(status.enum_variants.len(), 3);

        let user_service = &module.types[3];
        assert_eq!(user_service.name, "UserService");
        assert_eq!(user_service.kind, TypeKind::Class);

        // 2 exported functions
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions[0].name, "fetchUsers");
        assert_eq!(module.functions[0].visibility, Visibility::Public);
        assert_eq!(module.functions[1].name, "processUser");
    }

    #[test]
    fn test_ts_call_extraction() {
        let parser = JsTsParser::new();
        let source = r#"
class Service {
    process() {
        console.log("processing");
        this.validate();
        const result = helper();
        new Worker("task");
    }
}
"#;
        let path = Path::new("test.ts");
        let module = parser.parse_file(path, source).unwrap();

        let service = &module.types[0];
        let process = &service.methods[0];

        // Should find: console.log, this.validate, helper, Worker.<init>
        assert!(process.calls.iter().any(|c| c.target_method == "log"));
        assert!(process.calls.iter().any(|c| c.target_method == "validate"));
        assert!(process.calls.iter().any(|c| c.target_method == "helper" && c.target_type.is_none()));
        assert!(process.calls.iter().any(|c| c.target_method == "<init>" && c.target_type.as_deref() == Some("Worker")));
    }
}
