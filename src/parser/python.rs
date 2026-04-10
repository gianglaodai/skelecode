use std::cell::RefCell;
use std::path::Path;
use tree_sitter::{Node, Parser};

use super::LanguageParser;
use crate::ir::*;

pub struct PythonParser {
    parser: RefCell<Parser>,
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .expect("Error loading Python grammar");
        Self { parser: RefCell::new(parser) }
    }
}

impl LanguageParser for PythonParser {
    fn language(&self) -> Language {
        Language::Python
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "py")
    }

    fn parse_file(&self, path: &Path, source: &str) -> Option<Module> {
        let tree = self.parser.borrow_mut().parse(source, None)?;
        let root = tree.root_node();

        let module_path = path_to_module(path);
        let mut types = Vec::new();
        let mut functions = Vec::new();
        let mut imports = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "import_statement" => parse_import(child, source, &mut imports),
                "import_from_statement" => parse_from_import(child, source, &mut imports),
                "class_definition" => {
                    if let Some(td) = parse_class(child, source, &[]) {
                        types.push(td);
                    }
                }
                "function_definition" | "async_function_definition" => {
                    if let Some(f) = parse_free_function(child, source, &[]) {
                        functions.push(f);
                    }
                }
                "decorated_definition" => {
                    let decorators = extract_decorators(child, source);
                    if let Some(def) = find_definition(child) {
                        match def.kind() {
                            "class_definition" => {
                                if let Some(td) = parse_class(def, source, &decorators) {
                                    types.push(td);
                                }
                            }
                            "function_definition" | "async_function_definition" => {
                                if let Some(f) = parse_free_function(def, source, &decorators) {
                                    functions.push(f);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        if types.is_empty() && functions.is_empty() && imports.is_empty() {
            return None;
        }

        Some(Module {
            path: module_path,
            language: Language::Python,
            types,
            functions,
            imports,
        })
    }
}

// ── Class parsing ─────────────────────────────────────────────────────────────

fn parse_class(node: Node, source: &str, class_decorators: &[Annotation]) -> Option<TypeDef> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source).to_string();

    // Base classes → relations
    let mut relations = Vec::new();
    if let Some(args) = node.child_by_field_name("superclasses") {
        let mut c = args.walk();
        for child in args.children(&mut c) {
            match child.kind() {
                "identifier" | "attribute" => {
                    let base = node_text(child, source).to_string();
                    if base != "object" {
                        relations.push(TypeRelation {
                            kind: RelationKind::Extends,
                            target: base,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    // Class body: collect fields (class-level), methods, inner classes
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let body = node.child_by_field_name("body")?;

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "function_definition" | "async_function_definition" => {
                if let Some(m) = parse_method(child, source, &[]) {
                    // Extract __init__ fields before storing the method
                    if m.name == "__init__" {
                        let init_fields = extract_fields_from_init(child, source);
                        for f in init_fields {
                            if !fields.iter().any(|ef: &Field| ef.name == f.name) {
                                fields.push(f);
                            }
                        }
                    }
                    methods.push(m);
                }
            }
            "decorated_definition" => {
                let decorators = extract_decorators(child, source);
                if let Some(def) = find_definition(child) {
                    if matches!(def.kind(), "function_definition" | "async_function_definition") {
                        if let Some(m) = parse_method(def, source, &decorators) {
                            if m.name == "__init__" {
                                let init_fields = extract_fields_from_init(def, source);
                                for f in init_fields {
                                    if !fields.iter().any(|ef: &Field| ef.name == f.name) {
                                        fields.push(f);
                                    }
                                }
                            }
                            methods.push(m);
                        }
                    }
                }
            }
            // Class-level annotated assignments: field: Type = value
            "expression_statement" => {
                if let Some(ann) = child.child(0) {
                    if ann.kind() == "assignment" || ann.kind() == "annotated_assignment" {
                        if let Some(f) = parse_class_field(ann, source) {
                            if !fields.iter().any(|ef: &Field| ef.name == f.name) {
                                fields.push(f);
                            }
                        }
                    }
                }
            }
            "annotated_assignment" => {
                if let Some(f) = parse_class_field(child, source) {
                    if !fields.iter().any(|ef: &Field| ef.name == f.name) {
                        fields.push(f);
                    }
                }
            }
            _ => {}
        }
    }

    Some(TypeDef {
        name,
        kind: TypeKind::Class,
        visibility: Visibility::Public,
        fields,
        methods,
        relations,
        annotations: class_decorators.to_vec(),
        type_params: Vec::new(),
        enum_variants: Vec::new(),
    })
}

// ── Method parsing ────────────────────────────────────────────────────────────

fn parse_method(node: Node, source: &str, decorators: &[Annotation]) -> Option<Method> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source).to_string();

    let params = extract_params(node, source, true);
    let return_type = extract_return_type(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    let is_static = decorators.iter().any(|d| d.name == "staticmethod" || d.name == "classmethod");

    Some(Method {
        name,
        params,
        return_type,
        visibility: Visibility::Public,
        calls,
        callers: Vec::new(),
        annotations: decorators.to_vec(),
        is_static,
    })
}

// ── Free function parsing ─────────────────────────────────────────────────────

fn parse_free_function(node: Node, source: &str, _decorators: &[Annotation]) -> Option<Function> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source).to_string();

    // Skip private/dunder helpers that aren't meaningful at module level
    if name.starts_with("__") && name.ends_with("__") && name != "__init__" {
        return None;
    }

    let params = extract_params(node, source, false);
    let return_type = extract_return_type(node, source);

    let calls = if let Some(body) = node.child_by_field_name("body") {
        extract_calls(body, source)
    } else {
        Vec::new()
    };

    let visibility = if name.starts_with('_') {
        Visibility::Private
    } else {
        Visibility::Public
    };

    Some(Function {
        name,
        params,
        return_type,
        visibility,
        calls,
        callers: Vec::new(),
    })
}

// ── Field extraction ──────────────────────────────────────────────────────────

/// Extract `self.field_name: Type` or `self.field_name = ...` from __init__ body.
fn extract_fields_from_init(fn_node: Node, source: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    if let Some(body) = fn_node.child_by_field_name("body") {
        collect_self_assignments(body, source, &mut fields);
    }
    fields
}

fn collect_self_assignments(node: Node, source: &str, fields: &mut Vec<Field>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "expression_statement" => {
                if let Some(inner) = child.child(0) {
                    extract_self_assign(inner, source, fields);
                }
            }
            "assignment" | "annotated_assignment" | "augmented_assignment" => {
                extract_self_assign(child, source, fields);
            }
            // Recurse into if/for/with/try bodies
            "if_statement" | "for_statement" | "while_statement"
            | "with_statement" | "try_statement" | "block" => {
                collect_self_assignments(child, source, fields);
            }
            _ => {}
        }
    }
}

fn extract_self_assign(node: Node, source: &str, fields: &mut Vec<Field>) {
    if !matches!(node.kind(), "assignment" | "annotated_assignment" | "augmented_assignment") {
        return;
    }
    let left = match node.child_by_field_name("left") {
        Some(n) => n,
        None => return,
    };
    if left.kind() != "attribute" {
        return;
    }
    let obj = match left.child_by_field_name("object") {
        Some(n) => n,
        None => return,
    };
    if node_text(obj, source) != "self" {
        return;
    }
    let attr = match left.child_by_field_name("attribute") {
        Some(n) => n,
        None => return,
    };
    let field_name = node_text(attr, source).to_string();

    // Try to get type annotation (annotated_assignment: self.x: Type = val)
    let type_name = node
        .child_by_field_name("type")
        .map(|t| node_text(t, source).to_string())
        .unwrap_or_default();

    let visibility = if field_name.starts_with("__") {
        Visibility::Private
    } else if field_name.starts_with('_') {
        Visibility::Protected
    } else {
        Visibility::Public
    };

    fields.push(Field { name: field_name, type_name, visibility });
}

/// Class-level annotated assignment: `field: Type = value`
fn parse_class_field(node: Node, source: &str) -> Option<Field> {
    if node.kind() != "annotated_assignment" {
        return None;
    }
    let left = node.child_by_field_name("left")?;
    // Only plain identifiers (not self.x) at class level
    if left.kind() != "identifier" {
        return None;
    }
    let name = node_text(left, source).to_string();
    // Skip dunders like __slots__, __tablename__
    if name.starts_with("__") && name.ends_with("__") {
        return None;
    }
    let type_name = node
        .child_by_field_name("type")
        .map(|t| node_text(t, source).to_string())
        .unwrap_or_default();

    let visibility = if name.starts_with("__") {
        Visibility::Private
    } else if name.starts_with('_') {
        Visibility::Protected
    } else {
        Visibility::Public
    };

    Some(Field { name, type_name, visibility })
}

// ── Parameter extraction ──────────────────────────────────────────────────────

/// Extract function parameters. `skip_self` removes the first param if it's `self`/`cls`.
fn extract_params(fn_node: Node, source: &str, skip_self: bool) -> Vec<Param> {
    let params_node = match fn_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return Vec::new(),
    };

    let mut params = Vec::new();
    let mut first = true;

    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = node_text(child, source).to_string();
                if first && skip_self && (name == "self" || name == "cls") {
                    first = false;
                    continue;
                }
                first = false;
                params.push(Param { name, type_name: String::new() });
            }
            "typed_parameter" => {
                first = false;
                if let Some(p) = parse_typed_param(child, source) {
                    params.push(p);
                }
            }
            "default_parameter" => {
                first = false;
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    params.push(Param { name, type_name: String::new() });
                }
            }
            "typed_default_parameter" => {
                first = false;
                if let Some(p) = parse_typed_param(child, source) {
                    params.push(p);
                }
            }
            // *args / **kwargs — skip
            _ => { first = false; }
        }
    }

    params
}

fn parse_typed_param(node: Node, source: &str) -> Option<Param> {
    // tree-sitter-python uses no field name for the identifier; "annotation" for the type.
    // Fall back to searching children directly for robustness.
    let name_node = node.child_by_field_name("name")
        .or_else(|| {
            let mut c = node.walk();
            node.children(&mut c).find(|ch| ch.kind() == "identifier")
        })?;
    let name = node_text(name_node, source).to_string();
    let type_name = node
        .child_by_field_name("annotation")
        .or_else(|| node.child_by_field_name("type"))
        .map(|t| node_text(t, source).to_string())
        .unwrap_or_default();
    Some(Param { name, type_name })
}

fn extract_return_type(fn_node: Node, source: &str) -> Option<String> {
    fn_node
        .child_by_field_name("return_type")
        .map(|n| node_text(n, source).to_string())
}

// ── Call extraction ───────────────────────────────────────────────────────────

fn extract_calls(node: Node, source: &str) -> Vec<CallRef> {
    let mut calls = Vec::new();
    collect_calls(node, source, &mut calls);
    // Deduplicate
    calls.dedup();
    calls
}

fn collect_calls(node: Node, source: &str, calls: &mut Vec<CallRef>) {
    if node.kind() == "call" {
        if let Some(func) = node.child_by_field_name("function") {
            let call_ref = match func.kind() {
                "identifier" => Some(CallRef {
                    target_type: None,
                    target_method: node_text(func, source).to_string(),
                }),
                "attribute" => {
                    if let (Some(attr_node), Some(obj)) = (
                        func.child_by_field_name("attribute"),
                        func.child_by_field_name("object"),
                    ) {
                        let method = node_text(attr_node, source).to_string();
                        let target = match obj.kind() {
                            "identifier" => Some(node_text(obj, source).to_string()),
                            "attribute" => Some(node_text(obj, source).to_string()),
                            _ => None,
                        };
                        Some(CallRef { target_type: target, target_method: method })
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(cr) = call_ref {
                if !calls.contains(&cr) {
                    calls.push(cr);
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, source, calls);
    }
}

// ── Decorator extraction ──────────────────────────────────────────────────────

fn extract_decorators(decorated_node: Node, source: &str) -> Vec<Annotation> {
    let mut decorators = Vec::new();
    let mut cursor = decorated_node.walk();
    for child in decorated_node.children(&mut cursor) {
        if child.kind() == "decorator" {
            // Strip leading '@' and take the first identifier/attribute
            let text = node_text(child, source)
                .trim_start_matches('@')
                .trim()
                .to_string();
            // Take only the decorator name (no arguments)
            let name = text
                .split('(')
                .next()
                .unwrap_or(&text)
                .trim()
                .to_string();
            if !name.is_empty() {
                decorators.push(Annotation { name });
            }
        }
    }
    decorators
}

/// Find the actual class/function definition inside a decorated_definition node.
fn find_definition(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_definition"
            | "function_definition"
            | "async_function_definition" => return Some(child),
            _ => {}
        }
    }
    None
}

// ── Import parsing ────────────────────────────────────────────────────────────

/// `import foo.bar` → alias `bar`, qualified `foo.bar`
/// `import foo.bar as fb` → alias `fb`, qualified `foo.bar`
fn parse_import(node: Node, source: &str, imports: &mut Vec<ImportedName>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" => {
                let qualified = node_text(child, source).replace('.', "::");
                let alias = node_text(child, source)
                    .split('.')
                    .last()
                    .unwrap_or("")
                    .to_string();
                if !alias.is_empty() {
                    imports.push(ImportedName { alias, qualified });
                }
            }
            "aliased_import" => {
                let name_node = child.child_by_field_name("name");
                let alias_node = child.child_by_field_name("alias");
                if let (Some(name), Some(alias)) = (name_node, alias_node) {
                    imports.push(ImportedName {
                        alias: node_text(alias, source).to_string(),
                        qualified: node_text(name, source).replace('.', "::"),
                    });
                }
            }
            _ => {}
        }
    }
}

/// `from foo.bar import Baz` → alias `Baz`, qualified `foo.bar::Baz`
/// `from foo.bar import Baz as B` → alias `B`, qualified `foo.bar::Baz`
fn parse_from_import(node: Node, source: &str, imports: &mut Vec<ImportedName>) {
    // module_name field = dotted_name or relative_import
    let module_name_node = node.child_by_field_name("module_name");
    let module_prefix = module_name_node
        .map(|n| node_text(n, source).replace('.', "::"))
        .unwrap_or_default();

    let module_name_id = module_name_node.map(|n| n.id());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Skip the module_name node itself (already captured above), keywords, and wildcards
        if Some(child.id()) == module_name_id {
            continue;
        }
        match child.kind() {
            "wildcard_import" | "import_prefix" => {}
            "dotted_name" => {
                // This is one of the imported names (e.g. `User` in `from models import User`)
                let name = node_text(child, source).to_string();
                let alias = name.split('.').last().unwrap_or(&name).to_string();
                let qualified = if module_prefix.is_empty() {
                    name.replace('.', "::")
                } else {
                    format!("{}::{}", module_prefix, name.replace('.', "::"))
                };
                imports.push(ImportedName { alias, qualified });
            }
            "identifier" => {
                let name = node_text(child, source).to_string();
                if name == "import" || name == "from" {
                    continue;
                }
                let qualified = if module_prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", module_prefix, name)
                };
                imports.push(ImportedName { alias: name, qualified });
            }
            "aliased_import" => {
                let name_node = child.child_by_field_name("name");
                let alias_node = child.child_by_field_name("alias");
                if let (Some(name_n), Some(alias_n)) = (name_node, alias_node) {
                    let name = node_text(name_n, source).to_string();
                    let alias = node_text(alias_n, source).to_string();
                    let qualified = if module_prefix.is_empty() {
                        name.replace('.', "::")
                    } else {
                        format!("{}::{}", module_prefix, name.replace('.', "::"))
                    };
                    imports.push(ImportedName { alias, qualified });
                }
            }
            _ => {}
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

/// Convert a file path to a Python module path.
/// `src/models/user.py` → `src.models.user`
/// `src/models/__init__.py` → `src.models`
fn path_to_module(path: &Path) -> String {
    let components: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        return "unknown".to_string();
    }

    // Limit to last 6 components to avoid overly long paths from absolute roots
    let start = components.len().saturating_sub(6);
    let mut parts: Vec<String> = components[start..].to_vec();

    // Strip .py extension from last component
    if let Some(last) = parts.last_mut() {
        if let Some(stripped) = last.strip_suffix(".py") {
            *last = stripped.to_string();
        }
    }

    // __init__ represents the package itself
    if parts.last().map(|s| s.as_str()) == Some("__init__") {
        parts.pop();
    }

    parts.join(".")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Module {
        let parser = PythonParser::new();
        let path = Path::new("test.py");
        parser.parse_file(path, source).expect("parse failed")
    }

    #[test]
    fn test_basic_class() {
        let module = parse(r#"
class User:
    name: str
    _age: int

    def __init__(self, name: str, age: int):
        self.name = name
        self._age = age

    def greet(self) -> str:
        return f"Hello {self.name}"
"#);
        assert_eq!(module.types.len(), 1);
        let cls = &module.types[0];
        assert_eq!(cls.name, "User");
        assert_eq!(cls.kind, TypeKind::Class);

        // Fields from __init__ + class-level annotations
        assert!(cls.fields.iter().any(|f| f.name == "name"));
        assert!(cls.fields.iter().any(|f| f.name == "_age"));

        // Methods
        assert!(cls.methods.iter().any(|m| m.name == "greet"));
        let greet = cls.methods.iter().find(|m| m.name == "greet").unwrap();
        assert_eq!(greet.return_type.as_deref(), Some("str"));
    }

    #[test]
    fn test_inheritance() {
        let module = parse(r#"
class Admin(User, Auditable):
    def delete(self, uid: str) -> None:
        pass
"#);
        let cls = &module.types[0];
        assert_eq!(cls.relations.len(), 2);
        assert!(cls.relations.iter().any(|r| r.target == "User"));
        assert!(cls.relations.iter().any(|r| r.target == "Auditable"));
    }

    #[test]
    fn test_free_functions() {
        let module = parse(r#"
def create_user(name: str, age: int) -> User:
    return User(name, age)

def _helper():
    pass
"#);
        assert_eq!(module.functions.len(), 2);
        let create = module.functions.iter().find(|f| f.name == "create_user").unwrap();
        assert_eq!(create.params.len(), 2);
        assert_eq!(create.return_type.as_deref(), Some("User"));
        let helper = module.functions.iter().find(|f| f.name == "_helper").unwrap();
        assert_eq!(helper.visibility, Visibility::Private);
    }

    #[test]
    fn test_decorators_and_static() {
        let module = parse(r#"
class Service:
    @staticmethod
    def create() -> "Service":
        pass

    @classmethod
    def from_config(cls, config: dict) -> "Service":
        pass

    @property
    def name(self) -> str:
        return self._name
"#);
        let cls = &module.types[0];
        let create = cls.methods.iter().find(|m| m.name == "create").unwrap();
        assert!(create.is_static);
        let from_cfg = cls.methods.iter().find(|m| m.name == "from_config").unwrap();
        assert!(from_cfg.is_static);
        let name_prop = cls.methods.iter().find(|m| m.name == "name").unwrap();
        assert!(!name_prop.is_static);
        assert!(name_prop.annotations.iter().any(|a| a.name == "property"));
    }

    #[test]
    fn test_call_extraction() {
        let module = parse(r#"
class OrderService:
    def create(self, data: dict) -> Order:
        user = self.user_repo.find(data["user_id"])
        order = Order(user=user)
        self.notify(order)
        return order
"#);
        let cls = &module.types[0];
        let m = cls.methods.iter().find(|m| m.name == "create").unwrap();
        assert!(m.calls.iter().any(|c| c.target_type.as_deref() == Some("self.user_repo") && c.target_method == "find"));
        assert!(m.calls.iter().any(|c| c.target_type.is_none() && c.target_method == "Order"));
        assert!(m.calls.iter().any(|c| c.target_type.as_deref() == Some("self") && c.target_method == "notify"));
    }

    #[test]
    fn test_from_import() {
        let module = parse(r#"
from models.user import User, Admin
from services.auth import AuthService as Auth
"#);
        // No types/functions, module would be None unless we add a class
        // Test via a module that actually produces content
        let module2 = parse(r#"
from models.user import User
from services.auth import AuthService as Auth

def get_user() -> User:
    pass
"#);
        let imp = module2.imports.iter().find(|i| i.alias == "User").unwrap();
        assert_eq!(imp.qualified, "models::user::User");
        let imp2 = module2.imports.iter().find(|i| i.alias == "Auth").unwrap();
        assert_eq!(imp2.qualified, "services::auth::AuthService");
    }
}
