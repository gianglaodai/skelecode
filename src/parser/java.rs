use std::path::Path;
use tree_sitter::{Node, Parser};

use super::LanguageParser;
use crate::ir::*;

pub struct JavaParser {
    parser: std::cell::RefCell<Parser>,
}

impl Default for JavaParser {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .expect("Error loading Java grammar");
        Self {
            parser: std::cell::RefCell::new(parser),
        }
    }
}

impl LanguageParser for JavaParser {
    fn language(&self) -> Language {
        Language::Java
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "java")
    }

    fn parse_file(&self, path: &Path, source: &str) -> Option<Module> {
        let tree = self.parser.borrow_mut().parse(source, None)?;
        let root = tree.root_node();

        // Java path conventionally uses packages. We can extract package name from file text.
        let mut package_name = String::new();
        let mut types = Vec::new();
        let functions = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "package_declaration" => {
                    if let Some(pkg_name) = parse_package_name(child, source) {
                        package_name = pkg_name;
                    }
                }
                "class_declaration" => {
                    if let Some(td) = parse_class(child, source) {
                        types.push(td);
                    }
                }
                "interface_declaration" => {
                    if let Some(td) = parse_interface(child, source) {
                        types.push(td);
                    }
                }
                "enum_declaration" => {
                    if let Some(td) = parse_enum(child, source) {
                        types.push(td);
                    }
                }
                "record_declaration" => {
                    if let Some(td) = parse_record(child, source) {
                        types.push(td);
                    }
                }
                _ => {}
            }
        }

        // If package wasn't found, fallback to file path logic
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
            language: Language::Java,
            types,
            functions,
        })
    }
}

fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn parse_package_name(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
            return Some(node_text(child, source).to_string());
        }
    }
    None
}

struct Modifiers {
    visibility: Visibility,
    is_static: bool,
}

fn extract_modifiers(node: Node, source: &str) -> Modifiers {
    let mut vis = Visibility::Internal; // Package-private by default in Java
    let mut is_static = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for m in child.children(&mut mod_cursor) {
                match node_text(m, source) {
                    "public" => vis = Visibility::Public,
                    "private" => vis = Visibility::Private,
                    "protected" => vis = Visibility::Protected,
                    "static" => is_static = true,
                    _ => {}
                }
            }
            break;
        }
    }

    Modifiers {
        visibility: vis,
        is_static,
    }
}

fn extract_type_params(node: Node, source: &str) -> Vec<String> {
    if let Some(tp) = node.child_by_field_name("type_parameters") {
        let mut cursor = tp.walk();
        let mut params = Vec::new();
        for child in tp.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                params.push(node_text(child, source).to_string());
            }
        }
        return params;
    }
    Vec::new()
}

fn parse_class(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let mods = extract_modifiers(node, source);
    let type_params = extract_type_params(node, source);

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut relations = Vec::new();

    // Superclass
    if let Some(superclass) = node.child_by_field_name("superclass") {
        // superclass is usually "superclass" node with a type child
        relations.push(TypeRelation {
            kind: RelationKind::Extends,
            target: node_text(superclass, source).trim_start_matches("extends ").trim().to_string(),
        });
    }

    // Interfaces
    if let Some(interfaces) = node.child_by_field_name("interfaces") {
        let mut cursor = interfaces.walk();
        for child in interfaces.children(&mut cursor) {
            if child.kind() == "type_list" {
                let mut type_cursor = child.walk();
                for type_node in child.children(&mut type_cursor) {
                    if type_node.kind() == "type_identifier" || type_node.kind() == "generic_type" {
                        relations.push(TypeRelation {
                            kind: RelationKind::Implements,
                            target: node_text(type_node, source).to_string(),
                        });
                    }
                }
            }
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "field_declaration" => {
                    fields.extend(parse_fields(child, source));
                }
                "method_declaration" => {
                    if let Some(m) = parse_method(child, source) {
                        methods.push(m);
                    }
                }
                "constructor_declaration" => {
                    if let Some(m) = parse_constructor(child, source) {
                        methods.push(m);
                    }
                }
                _ => {}
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Class,
        visibility: mods.visibility,
        fields,
        methods,
        relations,
        annotations: Vec::new(),
        type_params,
        enum_variants: Vec::new(),
    })
}

fn parse_interface(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let mods = extract_modifiers(node, source);
    let type_params = extract_type_params(node, source);

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut relations = Vec::new();

    // Extended interfaces
    if let Some(interfaces) = node.child_by_field_name("interfaces") {
        let mut cursor = interfaces.walk();
        for child in interfaces.children(&mut cursor) {
            if child.kind() == "type_list" {
                let mut type_cursor = child.walk();
                for type_node in child.children(&mut type_cursor) {
                    if type_node.kind() == "type_identifier" || type_node.kind() == "generic_type" {
                        relations.push(TypeRelation {
                            kind: RelationKind::Extends,
                            target: node_text(type_node, source).to_string(),
                        });
                    }
                }
            }
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "constant_declaration" => {
                    fields.extend(parse_fields(child, source));
                }
                "method_declaration" => {
                    if let Some(m) = parse_method(child, source) {
                        methods.push(m);
                    }
                }
                _ => {}
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Interface,
        visibility: mods.visibility,
        fields,
        methods,
        relations,
        annotations: Vec::new(),
        type_params,
        enum_variants: Vec::new(),
    })
}

fn parse_enum(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let mods = extract_modifiers(node, source);

    let mut variants = Vec::new();
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "enum_constant" => {
                    if let Some(vname) = child.child_by_field_name("name") {
                        let variant_name = node_text(vname, source).to_string();
                        // Also include arguments if present (e.g. RED(255, 0, 0))
                        let mut has_args = false;
                        let mut inner_cursor = child.walk();
                        for vc in child.children(&mut inner_cursor) {
                            if vc.kind() == "argument_list" {
                                variants.push(format!("{}{}", variant_name, node_text(vc, source)));
                                has_args = true;
                                break;
                            }
                        }
                        if !has_args {
                            variants.push(variant_name);
                        }
                    }
                }
                "field_declaration" => {
                    fields.extend(parse_fields(child, source));
                }
                "method_declaration" => {
                    if let Some(m) = parse_method(child, source) {
                        methods.push(m);
                    }
                }
                "constructor_declaration" => {
                    if let Some(m) = parse_constructor(child, source) {
                        methods.push(m);
                    }
                }
                _ => {}
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Enum,
        visibility: mods.visibility,
        fields,
        methods,
        relations: Vec::new(),
        annotations: Vec::new(),
        type_params: Vec::new(),
        enum_variants: variants,
    })
}

fn parse_record(node: Node, source: &str) -> Option<TypeDef> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let mods = extract_modifiers(node, source);
    let type_params = extract_type_params(node, source);

    let mut fields = Vec::new();
    let mut methods = Vec::new();

    // Record components (which become fields and methods implicitly, but we capture them as fields)
    if let Some(params) = node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if child.kind() == "record_declaration_parameter" {
                if let (Some(type_node), Some(name_node)) = (
                    child.child_by_field_name("type"),
                    child.child_by_field_name("name"),
                ) {
                    fields.push(Field {
                        name: node_text(name_node, source).to_string(),
                        type_name: node_text(type_node, source).to_string(),
                        visibility: Visibility::Private, // record components are implicitly private final fields
                    });
                }
            }
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                // Record can have explicit fields (static), compact constructors, methods
                "field_declaration" => {
                    fields.extend(parse_fields(child, source));
                }
                "method_declaration" => {
                    if let Some(m) = parse_method(child, source) {
                        methods.push(m);
                    }
                }
                "compact_constructor_declaration" => {
                    // Similar to constructor but without params
                    if let Some(mname) = child.child_by_field_name("name") {
                        let calls = if let Some(body_node) = child.child_by_field_name("body") {
                            extract_calls(body_node, source)
                        } else {
                            Vec::new()
                        };
                        let m_mods = extract_modifiers(child, source);
                        methods.push(Method {
                            name: node_text(mname, source).to_string(),
                            params: Vec::new(),
                            return_type: None, // Constructors have no return type
                            visibility: m_mods.visibility,
                            calls,
                            annotations: Vec::new(),
                            is_static: false,
                        });
                    }
                }
                "constructor_declaration" => {
                    if let Some(m) = parse_constructor(child, source) {
                        methods.push(m);
                    }
                }
                _ => {}
            }
        }
    }

    Some(TypeDef {
        name: name_str,
        kind: TypeKind::Record,
        visibility: mods.visibility,
        fields,
        methods,
        relations: Vec::new(), // Records can implement interfaces, could be added later
        annotations: Vec::new(),
        type_params,
        enum_variants: Vec::new(),
    })
}

fn parse_fields(node: Node, source: &str) -> Vec<Field> {
    let mods = extract_modifiers(node, source);
    let mut fields = Vec::new();
    if let Some(type_node) = node.child_by_field_name("type") {
        let type_name = node_text(type_node, source).to_string();
        
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    fields.push(Field {
                        name: node_text(name_node, source).to_string(),
                        type_name: type_name.clone(),
                        visibility: mods.visibility.clone(),
                    });
                }
            }
        }
    }
    fields
}

fn parse_method(node: Node, source: &str) -> Option<Method> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let mods = extract_modifiers(node, source);

    let mut return_type = None;
    if let Some(ret) = node.child_by_field_name("type") {
        return_type = Some(node_text(ret, source).split_whitespace().collect::<Vec<_>>().join(" "));
    }

    let params = parse_parameters(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
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
        annotations: Vec::new(),
        is_static: mods.is_static,
    })
}

fn parse_constructor(node: Node, source: &str) -> Option<Method> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, source).to_string();
    let mods = extract_modifiers(node, source);
    
    let params = parse_parameters(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    Some(Method {
        name: name_str,
        params,
        return_type: None, // Constructors don't have return types
        visibility: mods.visibility,
        calls,
        annotations: Vec::new(),
        is_static: false, // Constructors evaluate as non-static
    })
}

fn parse_parameters(node: Node, source: &str) -> Vec<Param> {
    let mut params = Vec::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
                if let (Some(type_node), Some(name_node)) = (
                    child.child_by_field_name("type"),
                    child.child_by_field_name("name"),
                ) {
                    params.push(Param {
                        name: node_text(name_node, source).to_string(),
                        type_name: node_text(type_node, source).to_string(),
                    });
                }
            }
        }
    }
    params
}

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
    let raw = match node.kind() {
        "this" | "super" | "identifier" => node_text(node, source).to_string(),
        "field_access" => {
            if let (Some(obj), Some(field)) = (
                node.child_by_field_name("object"),
                node.child_by_field_name("field"),
            ) {
                let obj_text = node_text(obj, source);
                if obj_text == "this" {
                    return format!("this.{}", node_text(field, source).trim());
                }
                return resolve_receiver(obj, source);
            }
            node_text(node, source).to_string()
        }
        "method_invocation" => {
            // Unwind method chaining
            if let Some(obj) = node.child_by_field_name("object") {
                return resolve_receiver(obj, source);
            }
            "?".to_string()
        }
        _ => node_text(node, source).to_string(),
    };
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_calls(node: Node, source: &str, calls: &mut Vec<CallRef>) {
    match node.kind() {
        "method_invocation" => {
            let method_name = if let Some(name_node) = node.child_by_field_name("name") {
                node_text(name_node, source).split_whitespace().collect::<Vec<_>>().join(" ")
            } else {
                "".to_string()
            };

            if let Some(obj) = node.child_by_field_name("object") {
                // e.g. foo.bar()
                let receiver = resolve_receiver(obj, source);
                calls.push(CallRef {
                    target_type: Some(receiver),
                    target_method: method_name,
                });
            } else {
                // local method call e.g. doWork()
                calls.push(CallRef {
                    target_type: None,
                    target_method: method_name,
                });
            }
        }
        "object_creation_expression" => {
            // new Foo()
            if let Some(type_node) = node.child_by_field_name("type") {
                calls.push(CallRef {
                    target_type: Some(node_text(type_node, source).split_whitespace().collect::<Vec<_>>().join(" ")),
                    target_method: "<init>".to_string(),
                });
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, source, calls);
    }
}
