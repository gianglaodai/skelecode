use super::Renderer;
use crate::ir::*;

pub struct MachineRenderer;

impl Renderer for MachineRenderer {
    fn render(&self, project: &Project) -> String {
        let mut out = String::new();

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

            render_module_group(path, modules, &mut out);
        }

        out
    }
}

/// Render a group of modules sharing the same path (merged into one block).
fn render_module_group(path: &str, modules: &[&Module], out: &mut String) {
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
        Language::Rust => "@mod",
    };
    out.push_str(&format!("{} {}\n", mod_tag, path));

    for module in modules {
        for td in &module.types {
            render_type(td, out);
        }
        for func in &module.functions {
            render_function(func, out);
        }
    }
}

fn render_type(td: &TypeDef, out: &mut String) {
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
            .map(|f| format!("{}:{}", f.name, f.type_name))
            .collect();
        line.push_str(&format!(" {{{}}}", fields.join(", ")));
    }

    out.push_str(&line);
    out.push('\n');

    // Visibility
    if td.visibility != Visibility::Private {
        out.push_str(&format!("  @vis {}\n", td.visibility.as_str()));
    }

    // Enum variants
    if !td.enum_variants.is_empty() {
        out.push_str(&format!("  @enum {}\n", td.enum_variants.join(", ")));
    }

    // Annotations
    if !td.annotations.is_empty() {
        let anns: Vec<String> = td.annotations.iter().map(|a| a.name.clone()).collect();
        out.push_str(&format!("  @ann {}\n", anns.join(", ")));
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
        render_method(method, out);
    }
}

fn render_method(method: &Method, out: &mut String) {
    let params: Vec<String> = method.params.iter().map(|p| format!("{}", p)).collect();
    let mut line = format!("  @fn {}({})", method.name, params.join(", "));

    if let Some(ref ret) = method.return_type {
        line.push_str(&format!("->{}", ret));
    }

    if method.visibility != Visibility::Private {
        line.push_str(&format!(" @vis {}", method.visibility.as_str()));
    }

    if method.is_static {
        line.push_str(" @static");
    }

    if !method.annotations.is_empty() {
        let anns: Vec<String> = method.annotations.iter().map(|a| a.name.clone()).collect();
        line.push_str(&format!(" @ann {}", anns.join(", ")));
    }

    if !method.calls.is_empty() {
        let calls: Vec<String> = method.calls.iter().map(|c| format!("{}", c)).collect();
        line.push_str(&format!(" @calls[{}]", calls.join(", ")));
    }

    out.push_str(&line);
    out.push('\n');
}

fn render_function(func: &Function, out: &mut String) {
    let params: Vec<String> = func.params.iter().map(|p| format!("{}", p)).collect();
    let mut line = format!("@fn {}({})", func.name, params.join(", "));

    if let Some(ref ret) = func.return_type {
        line.push_str(&format!("->{}", ret));
    }

    if func.visibility != Visibility::Private {
        line.push_str(&format!(" @vis {}", func.visibility.as_str()));
    }

    if !func.calls.is_empty() {
        let calls: Vec<String> = func.calls.iter().map(|c| format!("{}", c)).collect();
        line.push_str(&format!(" @calls[{}]", calls.join(", ")));
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
                }],
            }],
        }
    }

    #[test]
    fn test_machine_output() {
        let renderer = MachineRenderer;
        let output = renderer.render(&sample_project());

        assert!(output.contains("@lang rust"));
        assert!(output.contains("@mod parser"));
        assert!(output.contains("@type Parser [struct] {source:String, pos:usize}"));
        assert!(output.contains("@vis pub"));
        assert!(output.contains("@impl Display"));
        assert!(output.contains("@fn new(source:String)->Self @vis pub @static"));
        assert!(
            output.contains("@fn parse()->Result<AST> @vis pub @calls[Lexer::tokenize, AST::new]")
        );
        assert!(output.contains("@fn helper()"));
    }
}
