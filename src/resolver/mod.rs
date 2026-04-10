/// Phase 7: Local Call Resolution (Level 1).
///
/// Resolves call receivers that are currently raw names (e.g., "self", "this",
/// "self.repo", "userRepo") into the actual type names they refer to, using
/// only information available within the same class/function scope.
///
/// Resolution rules (applied in order):
/// 1. `"self"` / `"this"` → the name of the current type
/// 2. `"self.X"` / `"this.X"` → the declared type of field `X` in the current type
/// 3. A name that matches a method parameter → that parameter's declared type
/// 4. A name that matches a field of the current type → that field's declared type
///
/// Any receiver not covered by the above rules is left unchanged, preserving
/// statically-known type names (e.g., `"Parser"` in `Parser::parse()`).
use std::collections::HashMap;

use crate::ir::*;

/// Phase 8: Cross-module call resolution via import statements.
///
/// For each module, builds an alias→qualified map from the parsed imports and
/// replaces any `call.target_type` whose value matches an imported alias with
/// the fully-qualified form `"{module_path}::{TypeName}"`.
///
/// This runs *after* `resolve_calls` (Phase 7), so `target_type` values
/// are already local names (e.g., `"Repository"`) rather than raw receivers
/// (e.g., `"self.repo"`).
///
/// The qualified form is chosen so that `sanitize_name(qualified)` matches the
/// Obsidian type-file stem `"{sanitized_mod}_{sanitized_type}"`, enabling
/// cross-file WikiLinks in Graph View.
pub fn resolve_import_calls(project: &mut Project) {
    for module in &mut project.modules {
        if module.imports.is_empty() {
            continue;
        }

        let import_map: HashMap<String, String> = module
            .imports
            .iter()
            .map(|i| (i.alias.clone(), i.qualified.clone()))
            .collect();

        for type_def in &mut module.types {
            for method in &mut type_def.methods {
                for call in &mut method.calls {
                    apply_import_map(call, &import_map);
                }
            }
        }
        for func in &mut module.functions {
            for call in &mut func.calls {
                apply_import_map(call, &import_map);
            }
        }
    }
}

fn apply_import_map(call: &mut CallRef, import_map: &HashMap<String, String>) {
    if let Some(ref target) = call.target_type {
        if let Some(qualified) = import_map.get(target.as_str()) {
            call.target_type = Some(qualified.clone());
        }
    }
}

/// Phase 10: Reverse Call Graph.
///
/// After Phases 7 & 8 have resolved all `target_type` values, this pass
/// walks every call edge and writes a `CallerRef` back into the target
/// method (or free function), giving each node its "called-by" list.
///
/// Matching strategy:
/// - Qualified names like `"com.example::Foo"` are matched by their last
///   segment (`"Foo"`), so they resolve to the same TypeDef regardless of
///   whether Phase 8 fully qualified them.
/// - Free-function calls (`target_type = None`) are matched by name against
///   free functions in the project.
pub fn resolve_reverse_calls(project: &mut Project) {
    // ── 1. Collect all forward edges ──────────────────────────────────────────
    // (source_type, source_method, target_type_simple, target_method)
    // source_type = None means the caller is a free function.
    let mut edges: Vec<(Option<String>, String, Option<String>, String)> = Vec::new();

    for module in &project.modules {
        for td in &module.types {
            for method in &td.methods {
                for call in &method.calls {
                    let simple_target = call.target_type.as_deref().map(last_segment);
                    edges.push((
                        Some(td.name.clone()),
                        method.name.clone(),
                        simple_target.map(str::to_string),
                        call.target_method.clone(),
                    ));
                }
            }
        }
        for func in &module.functions {
            for call in &func.calls {
                let simple_target = call.target_type.as_deref().map(last_segment);
                edges.push((
                    None,
                    func.name.clone(),
                    simple_target.map(str::to_string),
                    call.target_method.clone(),
                ));
            }
        }
    }

    // ── 2. Build type-name → (module_idx, type_idx) index ────────────────────
    let mut type_index: HashMap<String, (usize, usize)> = HashMap::new();
    for (mi, module) in project.modules.iter().enumerate() {
        for (ti, td) in module.types.iter().enumerate() {
            type_index.entry(td.name.clone()).or_insert((mi, ti));
        }
    }

    // ── 3. Build free-function name → (module_idx, fn_idx) index ─────────────
    let mut fn_index: HashMap<String, (usize, usize)> = HashMap::new();
    for (mi, module) in project.modules.iter().enumerate() {
        for (fi, func) in module.functions.iter().enumerate() {
            fn_index.entry(func.name.clone()).or_insert((mi, fi));
        }
    }

    // ── 4. Write reverse edges ────────────────────────────────────────────────
    for (source_type, source_method, target_type, target_method) in edges {
        let caller = CallerRef {
            source_type: source_type.clone(),
            source_method: source_method.clone(),
        };

        if let Some(ref ttype) = target_type {
            // Reverse into a method of a type
            if let Some(&(mi, ti)) = type_index.get(ttype.as_str()) {
                let type_def = &mut project.modules[mi].types[ti];
                for method in &mut type_def.methods {
                    if method.name == target_method && !method.callers.contains(&caller) {
                        method.callers.push(caller.clone());
                    }
                }
            }
        } else {
            // Reverse into a free function
            if let Some(&(mi, fi)) = fn_index.get(target_method.as_str()) {
                let func = &mut project.modules[mi].functions[fi];
                if !func.callers.contains(&caller) {
                    func.callers.push(caller);
                }
            }
        }
    }
}

/// Extract the last `::` segment from a (possibly qualified) type name.
fn last_segment(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

pub fn resolve_calls(project: &mut Project) {
    for module in &mut project.modules {
        for type_def in &mut module.types {
            resolve_type_def_calls(type_def);
        }
        for func in &mut module.functions {
            resolve_function_calls(func);
        }
    }
}

fn resolve_type_def_calls(type_def: &mut TypeDef) {
    let field_types: HashMap<String, String> = type_def
        .fields
        .iter()
        .map(|f| (f.name.clone(), base_type(&f.type_name)))
        .collect();

    let type_name = type_def.name.clone();

    for method in &mut type_def.methods {
        let param_types: HashMap<String, String> = method
            .params
            .iter()
            .filter(|p| !p.name.is_empty() && !p.type_name.is_empty())
            .map(|p| (p.name.clone(), base_type(&p.type_name)))
            .collect();

        for call in &mut method.calls {
            if let Some(resolved) =
                resolve_receiver(&call.target_type, &type_name, &field_types, &param_types)
            {
                call.target_type = Some(resolved);
            }
        }
    }
}

fn resolve_function_calls(func: &mut Function) {
    let param_types: HashMap<String, String> = func
        .params
        .iter()
        .filter(|p| !p.name.is_empty() && !p.type_name.is_empty())
        .map(|p| (p.name.clone(), base_type(&p.type_name)))
        .collect();

    for call in &mut func.calls {
        if let Some(resolved) =
            resolve_receiver(&call.target_type, "", &HashMap::new(), &param_types)
        {
            call.target_type = Some(resolved);
        }
    }
}

/// Attempt to resolve a raw receiver string to a concrete type name.
/// Returns `Some(resolved_type)` if resolution succeeded, `None` to leave
/// the existing `target_type` unchanged.
fn resolve_receiver(
    target_type: &Option<String>,
    current_type: &str,
    field_types: &HashMap<String, String>,
    param_types: &HashMap<String, String>,
) -> Option<String> {
    let receiver = target_type.as_deref()?;

    // Rule 1: self / this → current type
    if receiver == "self" || receiver == "this" {
        if !current_type.is_empty() {
            return Some(current_type.to_string());
        }
        return None;
    }

    // Rule 2: self.X / this.X → field type of X
    if let Some(field_name) = receiver
        .strip_prefix("self.")
        .or_else(|| receiver.strip_prefix("this."))
    {
        // Return None if field not found — keeps "self.X" intact rather than
        // replacing it with a wrong type.
        return field_types.get(field_name).cloned();
    }

    // Rule 3: param name → param type (checked before fields to avoid conflicts
    // when a param shadows a field with the same name)
    if let Some(t) = param_types.get(receiver) {
        return Some(t.clone());
    }

    // Rule 4: field name (without self prefix) → field type
    if let Some(t) = field_types.get(receiver) {
        return Some(t.clone());
    }

    // Not a local name — assume it is already a type name (e.g., "Parser",
    // "Result") and leave it unchanged.
    None
}

/// Strip language-specific type decorators down to a bare type name.
///
/// Applied transformations (in order):
/// 1. Rust reference markers: `&mut T`, `&T` → `T`
/// 2. Rust lifetime prefixes: `'a T` → `T`
/// 3. Kotlin nullable marker: `T?` → `T`
/// 4. Transparent single-argument wrappers (Rust): `Box<T>`, `Arc<T>`, `Rc<T>`,
///    `RefCell<T>`, `Option<T>`, `Mutex<T>`, `RwLock<T>`, `Cell<T>`, `Weak<T>` → `T`
///    (recursively, so `Box<Arc<Foo>>` → `Foo`)
/// 5. Generic parameters: `Foo<Bar>` → `Foo`
pub fn base_type(type_str: &str) -> String {
    let s = type_str.trim();

    // Strip Rust reference / mut markers
    let s = s
        .trim_start_matches("&mut ")
        .trim_start_matches("& mut ")
        .trim_start_matches("&");
    let s = s.trim();

    // Strip Rust lifetime prefixes like `'a `
    let s = if s.starts_with('\'') {
        s.splitn(2, ' ').nth(1).unwrap_or(s).trim()
    } else {
        s
    };

    // Strip Kotlin nullable `?`
    let s = s.trim_end_matches('?').trim();

    // Transparent single-arg wrappers
    const WRAPPERS: &[&str] = &[
        "Box", "Arc", "Rc", "RefCell", "Option", "Mutex", "RwLock", "Cell", "Weak",
    ];
    for wrapper in WRAPPERS {
        if let Some(inner) = s.strip_prefix(wrapper) {
            let inner = inner.trim();
            if inner.starts_with('<') && inner.ends_with('>') {
                let inner_type = &inner[1..inner.len() - 1];
                return base_type(inner_type); // recurse: Box<Arc<Foo>> → Foo
            }
        }
    }

    // Strip generic parameters: `Foo<Bar, Baz>` → `Foo`
    if let Some(idx) = s.find('<') {
        s[..idx].trim().to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ────────────────────────────────────────────────────────────────

    fn project_with(types: Vec<TypeDef>) -> Project {
        Project {
            modules: vec![Module {
                path: "test".to_string(),
                language: Language::Rust,
                types,
                functions: Vec::new(),
                imports: Vec::new(),
            }],
        }
    }

    fn project_with_func(func: Function) -> Project {
        Project {
            modules: vec![Module {
                path: "test".to_string(),
                language: Language::Rust,
                types: Vec::new(),
                functions: vec![func],
                imports: Vec::new(),
            }],
        }
    }

    fn project_with_imports(types: Vec<TypeDef>, imports: Vec<ImportedName>) -> Project {
        Project {
            modules: vec![Module {
                path: "test".to_string(),
                language: Language::Rust,
                types,
                functions: Vec::new(),
                imports,
            }],
        }
    }

    fn type_def(
        name: &str,
        fields: &[(&str, &str)],
        methods: Vec<Method>,
    ) -> TypeDef {
        TypeDef {
            name: name.to_string(),
            kind: TypeKind::Struct,
            visibility: Visibility::Public,
            fields: fields
                .iter()
                .map(|(n, t)| Field {
                    name: n.to_string(),
                    type_name: t.to_string(),
                    visibility: Visibility::Private,
                })
                .collect(),
            methods,
            relations: Vec::new(),
            annotations: Vec::new(),
            type_params: Vec::new(),
            enum_variants: Vec::new(),
        }
    }

    fn method(
        name: &str,
        params: &[(&str, &str)],
        calls: &[(&str, &str)], // (receiver, method_name)
    ) -> Method {
        Method {
            name: name.to_string(),
            params: params
                .iter()
                .map(|(n, t)| Param {
                    name: n.to_string(),
                    type_name: t.to_string(),
                })
                .collect(),
            return_type: None,
            visibility: Visibility::Public,
            calls: calls
                .iter()
                .map(|(recv, meth)| CallRef {
                    target_type: if recv.is_empty() {
                        None
                    } else {
                        Some(recv.to_string())
                    },
                    target_method: meth.to_string(),
                })
                .collect(),
            callers: Vec::new(),
            annotations: Vec::new(),
            is_static: false,
        }
    }

    fn resolved(project: &Project, type_idx: usize, method_idx: usize, call_idx: usize) -> Option<&str> {
        project.modules[0].types[type_idx].methods[method_idx].calls[call_idx]
            .target_type
            .as_deref()
    }

    // ── Rule 1: self / this ────────────────────────────────────────────────────

    #[test]
    fn test_resolve_self() {
        let m = method("process", &[], &[("self", "fetch")]);
        let mut p = project_with(vec![type_def("Service", &[], vec![m])]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("Service"));
    }

    #[test]
    fn test_resolve_this() {
        let m = method("process", &[], &[("this", "validate")]);
        let mut p = project_with(vec![type_def("UserService", &[], vec![m])]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("UserService"));
    }

    // ── Rule 2: self.X / this.X ───────────────────────────────────────────────

    #[test]
    fn test_resolve_self_field() {
        let m = method("save", &[], &[("self.repo", "save")]);
        let mut p = project_with(vec![type_def(
            "Service",
            &[("repo", "Repository")],
            vec![m],
        )]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("Repository"));
    }

    #[test]
    fn test_resolve_this_field() {
        let m = method("save", &[], &[("this.repo", "save")]);
        let mut p = project_with(vec![type_def(
            "Service",
            &[("repo", "UserRepository")],
            vec![m],
        )]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("UserRepository"));
    }

    #[test]
    fn test_self_field_not_found_keeps_original() {
        // self.unknown — field not present; keep "self.unknown" unchanged
        let m = method("act", &[], &[("self.unknown", "foo")]);
        let mut p = project_with(vec![type_def("Service", &[], vec![m])]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("self.unknown"));
    }

    // ── Rule 3: parameter name ─────────────────────────────────────────────────

    #[test]
    fn test_resolve_param() {
        let m = method(
            "process",
            &[("userRepo", "UserRepository")],
            &[("userRepo", "findById")],
        );
        let mut p = project_with(vec![type_def("Service", &[], vec![m])]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("UserRepository"));
    }

    #[test]
    fn test_param_shadows_field_with_same_name() {
        // Param wins over field when both have the same name
        let m = method(
            "process",
            &[("repo", "SpecialRepo")],
            &[("repo", "findById")],
        );
        let mut p = project_with(vec![type_def(
            "Service",
            &[("repo", "DefaultRepo")],
            vec![m],
        )]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("SpecialRepo"));
    }

    // ── Rule 4: bare field name ────────────────────────────────────────────────

    #[test]
    fn test_resolve_bare_field() {
        // Kotlin-style: `repo.findById()` where `repo` is a property
        let m = method("process", &[], &[("repo", "findById")]);
        let mut p = project_with(vec![type_def(
            "Service",
            &[("repo", "Repository")],
            vec![m],
        )]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("Repository"));
    }

    // ── Static / known type names unchanged ───────────────────────────────────

    #[test]
    fn test_known_type_unchanged() {
        // "Parser" is already a type name; resolver must not overwrite it
        let m = method("process", &[], &[("Parser", "parse")]);
        let mut p = project_with(vec![type_def("Service", &[], vec![m])]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("Parser"));
    }

    #[test]
    fn test_free_call_unchanged() {
        // target_type = None (plain function call) stays None
        let m = method("process", &[], &[("", "helper")]);
        let mut p = project_with(vec![type_def("Service", &[], vec![m])]);
        resolve_calls(&mut p);
        assert_eq!(
            p.modules[0].types[0].methods[0].calls[0].target_type,
            None
        );
    }

    // ── Free function param resolution ────────────────────────────────────────

    #[test]
    fn test_resolve_free_function_param() {
        let func = Function {
            name: "handle".to_string(),
            params: vec![Param {
                name: "svc".to_string(),
                type_name: "OrderService".to_string(),
            }],
            return_type: None,
            visibility: Visibility::Public,
            calls: vec![CallRef {
                target_type: Some("svc".to_string()),
                target_method: "create".to_string(),
            }],
            callers: Vec::new(),
        };
        let mut p = project_with_func(func);
        resolve_calls(&mut p);
        assert_eq!(
            p.modules[0].functions[0].calls[0].target_type.as_deref(),
            Some("OrderService")
        );
    }

    // ── base_type stripping ───────────────────────────────────────────────────

    #[test]
    fn test_base_type() {
        assert_eq!(base_type("Repository"), "Repository");
        assert_eq!(base_type("&Repository"), "Repository");
        assert_eq!(base_type("&mut Repository"), "Repository");
        assert_eq!(base_type("Repository?"), "Repository");          // Kotlin nullable
        assert_eq!(base_type("Box<Repository>"), "Repository");
        assert_eq!(base_type("Arc<Repository>"), "Repository");
        assert_eq!(base_type("Option<UserService>"), "UserService");
        assert_eq!(base_type("RefCell<Parser>"), "Parser");
        assert_eq!(base_type("Box<Arc<Foo>>"), "Foo");               // double-wrap
        assert_eq!(base_type("List<User>"), "List");                 // non-wrapper generic
        assert_eq!(base_type("HashMap<String, Value>"), "HashMap");
    }

    #[test]
    fn test_base_type_with_generic_field_resolves() {
        // repo: Arc<Repository> → field type strips to "Repository"
        let m = method("save", &[], &[("self.repo", "save")]);
        let mut p = project_with(vec![type_def(
            "Service",
            &[("repo", "Arc<Repository>")],
            vec![m],
        )]);
        resolve_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("Repository"));
    }

    // ── Integration: multiple calls in one method ─────────────────────────────

    #[test]
    fn test_multiple_calls_resolved() {
        let m = method(
            "process",
            &[("logger", "Logger")],
            &[
                ("self", "validate"),
                ("self.repo", "save"),
                ("logger", "info"),
                ("Parser", "parse"),  // already a type name
                ("", "helper"),       // free call
            ],
        );
        let mut p = project_with(vec![type_def(
            "Service",
            &[("repo", "UserRepository")],
            vec![m],
        )]);
        resolve_calls(&mut p);

        let calls = &p.modules[0].types[0].methods[0].calls;
        assert_eq!(calls[0].target_type.as_deref(), Some("Service"));
        assert_eq!(calls[1].target_type.as_deref(), Some("UserRepository"));
        assert_eq!(calls[2].target_type.as_deref(), Some("Logger"));
        assert_eq!(calls[3].target_type.as_deref(), Some("Parser"));
        assert_eq!(calls[4].target_type, None);
    }

    // ── Phase 8: import-based resolution ──────────────────────────────────────

    fn imp(alias: &str, qualified: &str) -> ImportedName {
        ImportedName {
            alias: alias.to_string(),
            qualified: qualified.to_string(),
        }
    }

    #[test]
    fn test_import_resolution_replaces_alias() {
        // After Phase 7: target_type = "Repository"
        // Import maps "Repository" → "com.example.repo::Repository"
        let m = method("save", &[], &[("Repository", "findById")]);
        let mut p = project_with_imports(
            vec![type_def("Service", &[], vec![m])],
            vec![imp("Repository", "com.example.repo::Repository")],
        );
        resolve_import_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("com.example.repo::Repository"));
    }

    #[test]
    fn test_import_resolution_unknown_name_unchanged() {
        // "Parser" is not in the import map — leave it as-is
        let m = method("process", &[], &[("Parser", "parse")]);
        let mut p = project_with_imports(
            vec![type_def("Service", &[], vec![m])],
            vec![imp("Repository", "com.example.repo::Repository")],
        );
        resolve_import_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("Parser"));
    }

    #[test]
    fn test_import_resolution_free_call_unchanged() {
        // target_type = None stays None
        let m = method("process", &[], &[("", "helper")]);
        let mut p = project_with_imports(
            vec![type_def("Service", &[], vec![m])],
            vec![imp("Helper", "com.example::Helper")],
        );
        resolve_import_calls(&mut p);
        assert_eq!(p.modules[0].types[0].methods[0].calls[0].target_type, None);
    }

    #[test]
    fn test_import_alias_resolution() {
        // import com.example.UserRepository as UR
        // call target "UR" → "com.example::UserRepository"
        let m = method("process", &[], &[("UR", "findById")]);
        let mut p = project_with_imports(
            vec![type_def("Service", &[], vec![m])],
            vec![imp("UR", "com.example::UserRepository")],
        );
        resolve_import_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("com.example::UserRepository"));
    }

    // ── Phase 10: Reverse Call Graph ──────────────────────────────────────────

    #[test]
    fn test_reverse_calls_method_to_method() {
        // OrderService::create calls Repository::save
        // → Repository::save.callers should contain OrderService::create
        let repo_method = method("save", &[], &[]);
        let order_method = method("create", &[], &[("Repository", "save")]);
        let mut p = Project {
            modules: vec![Module {
                path: "test".to_string(),
                language: Language::Rust,
                types: vec![
                    type_def("Repository", &[], vec![repo_method]),
                    type_def("OrderService", &[], vec![order_method]),
                ],
                functions: Vec::new(),
                imports: Vec::new(),
            }],
        };
        resolve_calls(&mut p);
        resolve_reverse_calls(&mut p);
        let callers = &p.modules[0].types[0].methods[0].callers;
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].source_type.as_deref(), Some("OrderService"));
        assert_eq!(callers[0].source_method, "create");
    }

    #[test]
    fn test_reverse_calls_free_function() {
        // Service::process calls free function helper()
        // → helper.callers should contain Service::process
        let svc_method = method("process", &[], &[("", "helper")]);
        let func = Function {
            name: "helper".to_string(),
            params: Vec::new(),
            return_type: None,
            visibility: Visibility::Public,
            calls: Vec::new(),
            callers: Vec::new(),
        };
        let mut p = Project {
            modules: vec![Module {
                path: "test".to_string(),
                language: Language::Rust,
                types: vec![type_def("Service", &[], vec![svc_method])],
                functions: vec![func],
                imports: Vec::new(),
            }],
        };
        resolve_reverse_calls(&mut p);
        let callers = &p.modules[0].functions[0].callers;
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].source_type.as_deref(), Some("Service"));
        assert_eq!(callers[0].source_method, "process");
    }

    #[test]
    fn test_reverse_calls_no_duplicates() {
        // Two methods in Service both call Repository::save
        // → save.callers should have 2 distinct entries
        let m1 = method("create", &[], &[("Repository", "save")]);
        let m2 = method("update", &[], &[("Repository", "save")]);
        let repo_method = method("save", &[], &[]);
        let mut p = Project {
            modules: vec![Module {
                path: "test".to_string(),
                language: Language::Rust,
                types: vec![
                    type_def("Repository", &[], vec![repo_method]),
                    type_def("Service", &[], vec![m1, m2]),
                ],
                functions: Vec::new(),
                imports: Vec::new(),
            }],
        };
        resolve_calls(&mut p);
        resolve_reverse_calls(&mut p);
        let callers = &p.modules[0].types[0].methods[0].callers;
        assert_eq!(callers.len(), 2);
    }

    #[test]
    fn test_phase7_and_phase8_combined() {
        // Phase 7 resolves self.repo → Repository (field type)
        // Phase 8 resolves Repository → com.example.repo::Repository (import)
        let m = method("save", &[], &[("self.repo", "save")]);
        let mut p = project_with_imports(
            vec![type_def(
                "Service",
                &[("repo", "Repository")],
                vec![m],
            )],
            vec![imp("Repository", "com.example.repo::Repository")],
        );
        resolve_calls(&mut p);
        resolve_import_calls(&mut p);
        assert_eq!(resolved(&p, 0, 0, 0), Some("com.example.repo::Repository"));
    }
}
