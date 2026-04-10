use std::path::Path;
use tree_sitter::{Node, Parser};

use super::LanguageParser;
use crate::ir::*;

pub struct KotlinParser {
    parser: std::cell::RefCell<Parser>,
}

impl Default for KotlinParser {
    fn default() -> Self {
        Self::new()
    }
}

impl KotlinParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_kotlin_ng::LANGUAGE.into())
            .expect("Error loading Kotlin grammar");
        Self {
            parser: std::cell::RefCell::new(parser),
        }
    }
}

impl LanguageParser for KotlinParser {
    fn language(&self) -> Language {
        Language::Kotlin
    }

    fn can_parse(&self, path: &Path) -> bool {
        if !path.extension().is_some_and(|e| e == "kt" || e == "kts") {
            return false;
        }
        // Exclude Gradle build scripts (build.gradle.kts, settings.gradle.kts, etc.)
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        !name.ends_with(".gradle.kts")
    }

    fn parse_file(&self, path: &Path, source: &str) -> Option<Module> {
        let tree = self.parser.borrow_mut().parse(source, None)?;
        let root = tree.root_node();

        let mut package_name = String::new();
        let mut types = Vec::new();
        let mut functions = Vec::new();
        let mut imports = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "package_header" => {
                    if let Some(pkg) = parse_package_header(child, source) {
                        package_name = pkg;
                    }
                }
                "import_header" => {
                    imports.extend(parse_kotlin_import(child, source));
                }
                "class_declaration" => {
                    if let Some(td) = parse_class(child, source) {
                        types.push(td);
                    }
                }
                "object_declaration" => {
                    if let Some(td) = parse_object(child, source) {
                        types.push(td);
                    }
                }
                "function_declaration" => {
                    if let Some(f) = parse_top_level_function(child, source) {
                        functions.push(f);
                    }
                }
                _ => {}
            }
        }

        let module_path = if !package_name.is_empty() {
            package_name
        } else {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        };

        Some(Module {
            path: module_path,
            language: Language::Kotlin,
            types,
            functions,
            imports,
        })
    }
}

// ─── Import parsing ─────────────────────────────────────────────────────────────

fn parse_kotlin_import(node: Node, source: &str) -> Vec<ImportedName> {
    let raw = node_text(node, source).trim();
    let raw = raw.strip_prefix("import").unwrap_or(raw).trim();

    // Alias: "a.b.C as D"
    if let Some(as_idx) = raw.find(" as ") {
        let path = raw[..as_idx].trim();
        let alias = raw[as_idx + 4..].trim();
        return dot_qualified(path, Some(alias))
            .map(|i| vec![i])
            .unwrap_or_default();
    }

    // Wildcard
    if raw.ends_with(".*") {
        return Vec::new();
    }

    dot_qualified(raw, None).map(|i| vec![i]).unwrap_or_default()
}

/// Split `"a.b.ClassName"` at the last dot into `module_path::ClassName`.
fn dot_qualified(path: &str, alias_override: Option<&str>) -> Option<ImportedName> {
    let dot = path.rfind('.')?;
    let module_path = &path[..dot];
    let type_name = &path[dot + 1..];
    if !type_name.starts_with(|c: char| c.is_uppercase()) {
        return None;
    }
    let alias = alias_override.unwrap_or(type_name);
    Some(ImportedName {
        alias: alias.to_string(),
        qualified: format!("{}::{}", module_path, type_name),
    })
}

// ─── Helpers ────────────────────────────────────────────────────────────────────

fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn parse_package_header(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "qualified_identifier" || child.kind() == "identifier" {
            return Some(node_text(child, source).to_string());
        }
    }
    None
}

// ─── Modifiers ──────────────────────────────────────────────────────────────────

struct Modifiers {
    visibility: Visibility,
    is_static: bool,
    is_data: bool,
    is_sealed: bool,
    #[allow(dead_code)]
    is_abstract: bool,
}

fn extract_modifiers(node: Node, source: &str) -> Modifiers {
    let mut vis = Visibility::Public; // Kotlin default is public
    let is_static = false;
    let mut is_data = false;
    let mut is_sealed = false;
    let mut is_abstract = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for m in child.children(&mut mod_cursor) {
                match m.kind() {
                    "visibility_modifier" => match node_text(m, source) {
                        "public" => vis = Visibility::Public,
                        "private" => vis = Visibility::Private,
                        "protected" => vis = Visibility::Protected,
                        "internal" => vis = Visibility::Internal,
                        _ => {}
                    },
                    "class_modifier" => match node_text(m, source) {
                        "data" => is_data = true,
                        "sealed" => is_sealed = true,
                        _ => {}
                    },
                    "inheritance_modifier" => {
                        if node_text(m, source) == "abstract" {
                            is_abstract = true;
                        }
                    }
                    "member_modifier" => {
                        // companion objects make members effectively static
                    }
                    _ => {}
                }
            }
        }
    }

    // In Kotlin, top-level functions behave like static
    // We don't set is_static here for classes; it's for companion object members
    Modifiers {
        visibility: vis,
        is_static,
        is_data,
        is_sealed,
        is_abstract,
    }
}

fn extract_annotations(node: Node, source: &str) -> Vec<Annotation> {
    let mut annotations = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for m in child.children(&mut mod_cursor) {
                if m.kind() == "annotation" {
                    let text = node_text(m, source).trim().to_string();
                    annotations.push(Annotation { name: text });
                }
            }
        }
    }
    annotations
}

fn extract_type_params(node: Node, source: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_parameters" {
            let mut tp_cursor = child.walk();
            for tp in child.children(&mut tp_cursor) {
                if tp.kind() == "type_parameter" {
                    params.push(node_text(tp, source).to_string());
                }
            }
            break;
        }
    }
    params
}

// ─── Class Parsing ──────────────────────────────────────────────────────────────

fn parse_class(node: Node, source: &str) -> Option<TypeDef> {
    let mods = extract_modifiers(node, source);
    let annotations = extract_annotations(node, source);
    let type_params = extract_type_params(node, source);

    // Determine the kind
    let kind = if mods.is_data {
        TypeKind::DataClass
    } else if mods.is_sealed {
        TypeKind::SealedClass
    } else {
        // Check if it's an interface
        let mut is_interface = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "interface" {
                is_interface = true;
                break;
            }
        }
        if is_interface {
            TypeKind::Interface
        } else {
            TypeKind::Class
        }
    };

    // Find the name via simple_identifier child
    let name_str = find_class_name(node, source)?;

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut relations = Vec::new();
    let mut enum_variants = Vec::new();

    // Primary constructor parameters (val/var become properties)
    extract_primary_constructor_params(node, source, &mut fields);

    // Delegation specifiers (supertypes)
    extract_delegation_specifiers(node, source, &mut relations, kind == TypeKind::Interface);

    // Class body
    if let Some(body) = find_child_by_kind(node, "class_body") {
        parse_class_body(body, source, &mut fields, &mut methods, &mut enum_variants);
    }

    // Enum entries
    if let Some(body) = find_child_by_kind(node, "enum_class_body") {
        parse_enum_class_body(body, source, &mut fields, &mut methods, &mut enum_variants);
    }

    // Detect enum: if the node text starts with modifiers then "enum class"
    let final_kind = if has_enum_keyword(node, source) {
        TypeKind::Enum
    } else {
        kind
    };

    Some(TypeDef {
        name: name_str,
        kind: final_kind,
        visibility: mods.visibility,
        fields,
        methods,
        relations,
        annotations,
        type_params,
        enum_variants,
    })
}

fn has_enum_keyword(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for m in child.children(&mut mod_cursor) {
                if m.kind() == "class_modifier" && node_text(m, source) == "enum" {
                    return true;
                }
            }
        }
    }
    false
}

fn find_class_name(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" || child.kind() == "identifier" {
            let text = node_text(child, source);
            // Skip keywords
            if text != "class" && text != "interface" && text != "object"
                && text != "enum" && text != "fun" && text != "val" && text != "var"
            {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|child| child.kind() == kind)
}

fn extract_primary_constructor_params(node: Node, source: &str, fields: &mut Vec<Field>) {
    if let Some(ctor) = find_child_by_kind(node, "primary_constructor") {
        // class_parameters is the direct child containing the params
        let params_node = find_child_by_kind(ctor, "class_parameters").unwrap_or(ctor);
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "class_parameter" {
                // Only val/var parameters become properties
                let has_val_var = has_child_kind(child, "val") || has_child_kind(child, "var");
                if has_val_var {
                    let param_mods = extract_modifiers(child, source);
                    let name = find_child_text_by_kind(child, "identifier", source);
                    let type_name = find_type_text(child, source);
                    if let Some(n) = name {
                        fields.push(Field {
                            name: n,
                            type_name: type_name.unwrap_or_else(|| "Any".to_string()),
                            visibility: param_mods.visibility,
                        });
                    }
                }
            }
        }
    }
}

fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| c.kind() == kind)
}

fn find_child_text_by_kind(node: Node, kind: &str, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(node_text(child, source).to_string());
        }
    }
    None
}

fn find_type_text(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "user_type" || child.kind() == "nullable_type"
            || child.kind() == "function_type"
        {
            return Some(node_text(child, source).to_string());
        }
    }
    None
}

fn extract_delegation_specifiers(
    node: Node,
    source: &str,
    relations: &mut Vec<TypeRelation>,
    is_interface: bool,
) {
    // delegation_specifiers (plural) is the container node
    if let Some(specs) = find_child_by_kind(node, "delegation_specifiers") {
        let mut cursor = specs.walk();
        for child in specs.children(&mut cursor) {
            if child.kind() == "delegation_specifier" {
                add_relation_from_specifier(child, source, relations, is_interface);
            }
        }
    }

    // Deduplicate
    relations.dedup_by(|a, b| a.target == b.target && a.kind == b.kind);
}

fn add_relation_from_specifier(
    node: Node,
    source: &str,
    relations: &mut Vec<TypeRelation>,
    is_interface: bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "user_type" => {
                let target = extract_user_type_name(child, source);
                // First delegation specifier is extends for classes, all are extends for interfaces
                let kind = if is_interface {
                    RelationKind::Extends
                } else if relations.is_empty() {
                    // Heuristic: first one is likely extends (class), rest are implements
                    RelationKind::Extends
                } else {
                    RelationKind::Implements
                };
                relations.push(TypeRelation { kind, target });
            }
            "constructor_invocation" => {
                // Super class with constructor call, e.g., `class Foo : Bar()`
                if let Some(ut) = find_child_by_kind(child, "user_type") {
                    let target = extract_user_type_name(ut, source);
                    relations.push(TypeRelation {
                        kind: RelationKind::Extends,
                        target,
                    });
                }
            }
            _ => {}
        }
    }
}

fn extract_user_type_name(node: Node, source: &str) -> String {
    // user_type contains simple_user_type(s) joined by dots
    let text = node_text(node, source).to_string();
    // Strip generic parameters for the relation target
    if let Some(idx) = text.find('<') {
        text[..idx].trim().to_string()
    } else {
        text.trim().to_string()
    }
}

fn parse_class_body(
    body: Node,
    source: &str,
    fields: &mut Vec<Field>,
    methods: &mut Vec<Method>,
    _enum_variants: &mut Vec<String>,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "property_declaration" => {
                if let Some(f) = parse_property(child, source) {
                    fields.push(f);
                }
            }
            "function_declaration" => {
                if let Some(m) = parse_method(child, source) {
                    methods.push(m);
                }
            }
            "companion_object" => {
                parse_companion_object(child, source, fields, methods);
            }
            "class_declaration" => {
                // Nested class — skip for now (could be added later)
            }
            "object_declaration" => {
                // Nested object — skip
            }
            "secondary_constructor" => {
                if let Some(m) = parse_secondary_constructor(child, source) {
                    methods.push(m);
                }
            }
            _ => {}
        }
    }
}

fn parse_enum_class_body(
    body: Node,
    source: &str,
    fields: &mut Vec<Field>,
    methods: &mut Vec<Method>,
    enum_variants: &mut Vec<String>,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "enum_entry" => {
                if let Some(name) = find_child_text_by_kind(child, "identifier", source) {
                    // Check for arguments
                    if let Some(args) = find_child_by_kind(child, "value_arguments") {
                        enum_variants.push(format!("{}{}", name, node_text(args, source)));
                    } else {
                        enum_variants.push(name);
                    }
                }
            }
            "property_declaration" => {
                if let Some(f) = parse_property(child, source) {
                    fields.push(f);
                }
            }
            "function_declaration" => {
                if let Some(m) = parse_method(child, source) {
                    methods.push(m);
                }
            }
            "companion_object" => {
                parse_companion_object(child, source, fields, methods);
            }
            _ => {}
        }
    }
}

// ─── Object Declaration ─────────────────────────────────────────────────────────

fn parse_object(node: Node, source: &str) -> Option<TypeDef> {
    let name_str = find_class_name(node, source)?;
    let mods = extract_modifiers(node, source);
    let annotations = extract_annotations(node, source);

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut relations = Vec::new();

    extract_delegation_specifiers(node, source, &mut relations, false);

    if let Some(body) = find_child_by_kind(node, "class_body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "property_declaration" => {
                    if let Some(f) = parse_property(child, source) {
                        fields.push(f);
                    }
                }
                "function_declaration" => {
                    if let Some(mut m) = parse_method(child, source) {
                        m.is_static = true; // Object members are effectively static
                        methods.push(m);
                    }
                }
                _ => {}
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Object,
        visibility: mods.visibility,
        fields,
        methods,
        relations,
        annotations,
        type_params: Vec::new(),
        enum_variants: Vec::new(),
    })
}

// ─── Properties ─────────────────────────────────────────────────────────────────

fn parse_property(node: Node, source: &str) -> Option<Field> {
    let mods = extract_modifiers(node, source);

    // Find variable declaration: simple_identifier
    let mut name: Option<String> = None;
    let mut type_name: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declaration" {
            name = find_child_text_by_kind(child, "identifier", source);
            // Type after the colon
            type_name = find_type_in_node(child, source);
        }
    }

    // Fallback: direct simple_identifier child
    if name.is_none() {
        name = find_child_text_by_kind(node, "identifier", source);
        if type_name.is_none() {
            type_name = find_type_in_node(node, source);
        }
    }

    let n = name?;

    Some(Field {
        name: n,
        type_name: type_name.unwrap_or_else(|| "Any".to_string()),
        visibility: mods.visibility,
    })
}

fn find_type_in_node(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "user_type" || child.kind() == "nullable_type"
            || child.kind() == "function_type"
        {
            return Some(node_text(child, source).to_string());
        }
    }
    None
}

// ─── Methods / Functions ────────────────────────────────────────────────────────

fn parse_method(node: Node, source: &str) -> Option<Method> {
    let name_str = find_child_text_by_kind(node, "identifier", source)?;
    let mods = extract_modifiers(node, source);
    let annotations = extract_annotations(node, source);

    let mut return_type: Option<String> = None;
    let mut params = Vec::new();

    let mut cursor = node.walk();
    let mut found_params = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" && !found_params {
            params = parse_function_params(child, source);
            found_params = true;
        }
        // Return type comes after ":"
        if child.kind() == "user_type" || child.kind() == "nullable_type"
            || child.kind() == "function_type"
        {
            // Only set if we've already seen the params (to avoid confusing with receiver type)
            if found_params {
                return_type = Some(node_text(child, source).to_string());
            }
        }
    }

    let calls = if let Some(body) = find_child_by_kind(node, "function_body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Method {
        name: name_str,
        params,
        return_type,
        visibility: mods.visibility,
        calls,
        callers: Vec::new(),
        annotations,
        is_static: mods.is_static,
    })
}

fn parse_top_level_function(node: Node, source: &str) -> Option<Function> {
    let name_str = find_child_text_by_kind(node, "identifier", source)?;
    let mods = extract_modifiers(node, source);

    let mut return_type: Option<String> = None;
    let mut params = Vec::new();

    let mut cursor = node.walk();
    let mut found_params = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" && !found_params {
            params = parse_function_params(child, source);
            found_params = true;
        }
        if (child.kind() == "user_type" || child.kind() == "nullable_type"
            || child.kind() == "function_type") && found_params
        {
            return_type = Some(node_text(child, source).to_string());
        }
    }

    let calls = if let Some(body) = find_child_by_kind(node, "function_body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Function {
        name: name_str,
        params,
        return_type,
        visibility: mods.visibility,
        calls,
        callers: Vec::new(),
    })
}

fn parse_function_params(node: Node, source: &str) -> Vec<Param> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameter" {
            let name = find_child_text_by_kind(child, "identifier", source)
                .unwrap_or_default();
            let type_name = find_type_in_node(child, source)
                .unwrap_or_else(|| "Any".to_string());
            params.push(Param { name, type_name });
        }
    }
    params
}

// ─── Secondary Constructor ──────────────────────────────────────────────────────

fn parse_secondary_constructor(node: Node, source: &str) -> Option<Method> {
    let mods = extract_modifiers(node, source);

    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            params = parse_function_params(child, source);
            break;
        }
    }

    let calls = if let Some(body) = find_child_by_kind(node, "statements") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Method {
        name: "<init>".to_string(),
        params,
        return_type: None,
        visibility: mods.visibility,
        calls,
        callers: Vec::new(),
        annotations: Vec::new(),
        is_static: false,
    })
}

// ─── Companion Object ───────────────────────────────────────────────────────────

fn parse_companion_object(
    node: Node,
    source: &str,
    fields: &mut Vec<Field>,
    methods: &mut Vec<Method>,
) {
    if let Some(body) = find_child_by_kind(node, "class_body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "property_declaration" => {
                    if let Some(f) = parse_property(child, source) {
                        fields.push(f);
                    }
                }
                "function_declaration" => {
                    if let Some(mut m) = parse_method(child, source) {
                        m.is_static = true; // Companion object members are static
                        methods.push(m);
                    }
                }
                _ => {}
            }
        }
    }
}

// ─── Call Extraction ────────────────────────────────────────────────────────────

fn extract_calls(node: Node, source: &str) -> Vec<CallRef> {
    let mut calls = Vec::new();
    collect_calls(node, source, &mut calls);

    calls.sort_by(|a, b| format!("{}", a).cmp(&format!("{}", b)));
    calls.dedup_by(|a, b| format!("{}", a) == format!("{}", b));

    calls
}

fn collect_calls(node: Node, source: &str, calls: &mut Vec<CallRef>) {
    if node.kind() == "call_expression" {
        // e.g., foo.bar() or bar()
        if let Some(callee) = node.child(0) {
            match callee.kind() {
                "navigation_expression" => {
                    // e.g., foo.bar
                    let receiver = resolve_receiver(callee, source);
                    let method = find_last_simple_identifier(callee, source)
                        .unwrap_or_default();
                    if !method.is_empty() {
                        calls.push(CallRef {
                            target_type: Some(receiver),
                            target_method: method,
                        });
                    }
                }
                "identifier" => {
                    // Local function call
                    let method = node_text(callee, source).to_string();
                    calls.push(CallRef {
                        target_type: None,
                        target_method: method,
                    });
                }
                _ => {}
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, source, calls);
    }
}

fn resolve_receiver(node: Node, source: &str) -> String {
    // navigation_expression has children: expression "." simple_identifier
    if let Some(first_child) = node.child(0) {
        match first_child.kind() {
            "identifier" | "this_expression" => {
                return node_text(first_child, source).to_string();
            }
            "navigation_expression" => {
                return resolve_receiver(first_child, source);
            }
            "call_expression" => {
                // chained call: foo.bar().baz()
                if let Some(inner) = first_child.child(0) {
                    if inner.kind() == "navigation_expression" {
                        return resolve_receiver(inner, source);
                    }
                    return node_text(inner, source).to_string();
                }
            }
            _ => {}
        }
    }
    node_text(node, source)
        .split('.')
        .next()
        .unwrap_or("?")
        .to_string()
}

fn find_last_simple_identifier(node: Node, source: &str) -> Option<String> {
    let mut last = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            last = Some(node_text(child, source).to_string());
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kotlin_parser_basic() {
        let parser = KotlinParser::new();
        let source = r#"package com.example

class Foo(val name: String, val age: Int) : Bar(), Serializable {
    private val cache: Map<String, Any> = mutableMapOf()

    fun greet(): String {
        return name.uppercase()
    }

    fun process(input: String): Boolean {
        val result = cache.get(input)
        return result != null
    }

    companion object {
        fun create(name: String): Foo = Foo(name, 0)
    }
}

data class UserDto(val id: Long, val name: String, val email: String)

enum class Status(val code: Int) {
    ACTIVE(1),
    INACTIVE(0);

    fun isActive(): Boolean = this == ACTIVE
}

interface Repository {
    fun findById(id: Long): Any?
    fun save(entity: Any): Any
}

object AppConfig {
    val maxRetries: Int = 3
    fun getTimeout(): Long = 5000
}

fun topLevel(input: String): String {
    return input.trim()
}
"#;
        let path = std::path::Path::new("test.kt");
        let module = parser.parse_file(path, source).unwrap();

        assert_eq!(module.path, "com.example");
        assert_eq!(module.language, Language::Kotlin);
        assert!(module.types.len() >= 5);

        // class with fields, methods, relations
        let foo = module.types.iter().find(|t| t.name == "Foo").unwrap();
        assert_eq!(foo.kind, TypeKind::Class);
        assert!(foo.fields.len() >= 2);
        assert!(foo.methods.len() >= 2);
        assert!(foo.relations.iter().any(|r| r.target == "Bar" && r.kind == RelationKind::Extends));
        assert!(foo.relations.iter().any(|r| r.target == "Serializable"));

        // companion object method is static
        let create = foo.methods.iter().find(|m| m.name == "create").unwrap();
        assert!(create.is_static);

        // data class
        let dto = module.types.iter().find(|t| t.name == "UserDto").unwrap();
        assert_eq!(dto.kind, TypeKind::DataClass);
        assert_eq!(dto.fields.len(), 3);

        // enum
        let status = module.types.iter().find(|t| t.name == "Status").unwrap();
        assert_eq!(status.kind, TypeKind::Enum);
        assert!(status.enum_variants.len() >= 2);
        assert!(status.methods.iter().any(|m| m.name == "isActive"));

        // interface
        let repo = module.types.iter().find(|t| t.name == "Repository").unwrap();
        assert_eq!(repo.kind, TypeKind::Interface);
        assert_eq!(repo.methods.len(), 2);

        // object
        let config = module.types.iter().find(|t| t.name == "AppConfig").unwrap();
        assert_eq!(config.kind, TypeKind::Object);
        assert!(config.methods.iter().all(|m| m.is_static));

        // top-level function
        assert!(module.functions.iter().any(|f| f.name == "topLevel"));
    }

    #[test]
    fn test_kotlin_call_extraction() {
        let parser = KotlinParser::new();
        let source = r#"package com.test

class Service(val repo: Repository) {
    fun process(id: Long): Result {
        val entity = repo.findById(id)
        val validated = validate(entity)
        return Result.success(validated)
    }

    private fun validate(entity: Any): Boolean {
        return entity.toString().isNotEmpty()
    }
}
"#;
        let path = std::path::Path::new("test.kt");
        let module = parser.parse_file(path, source).unwrap();

        let svc = module.types.iter().find(|t| t.name == "Service").unwrap();
        let process = svc.methods.iter().find(|m| m.name == "process").unwrap();
        assert!(process.calls.iter().any(|c| c.target_method == "findById"));
        assert!(process.calls.iter().any(|c| c.target_method == "validate" && c.target_type.is_none()));
        assert!(process.calls.iter().any(|c| c.target_method == "success"));
    }
}
