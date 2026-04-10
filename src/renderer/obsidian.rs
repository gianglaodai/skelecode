use std::path::PathBuf;
use super::{Renderer, RenderOutput};
use crate::ir::*;

pub struct ObsidianRenderer;

/// Public wrapper — renders a single type to Obsidian markdown (for TUI preview).
pub fn render_type_file_pub(td: &TypeDef, module: &Module) -> String {
    let mod_name = sanitize_name(&module.path);
    let type_file_name = format!("{}_{}", mod_name, sanitize_name(&td.name));
    render_type_file(td, module, &mod_name, &type_file_name)
}

impl Renderer for ObsidianRenderer {
    fn render(&self, project: &Project) -> RenderOutput {
        let mut files = Vec::new();

        // ── Group modules by path ─────────────────────────────────────────────
        // Multiple source files in the same Kotlin/Java package all share the
        // same `module.path`. Grouping them prevents duplicate Index entries
        // and module-file collisions.
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

        // ── Index.md ──────────────────────────────────────────────────────────
        let mut index_content = String::from(
            "---\ntags:\n  - index\ncreated-by: skelecode\n---\n# Project Index\n\n",
        );
        index_content.push_str("## Modules\n");

        for (path, modules) in seen_paths.iter().zip(groups.iter()) {
            let mod_name = sanitize_name(path);
            // One entry per unique path
            index_content.push_str(&format!("- contains:: [[{}]]\n", mod_name));

            let first = modules[0];

            // ── Module file ───────────────────────────────────────────────────
            let mut mod_content = format!(
                "---\ntags:\n  - module\n  - {lang}\nlanguage: {lang}\nmodule: \"{path}\"\n---\n# {path}\n\n",
                lang = first.language.as_str(),
                path = path,
            );

            // Collect types and functions across all source files in the group
            let all_types: Vec<(&TypeDef, &Module)> = modules
                .iter()
                .flat_map(|m| m.types.iter().map(move |td| (td, *m)))
                .collect();

            let all_functions: Vec<&Function> = modules
                .iter()
                .flat_map(|m| m.functions.iter())
                .collect();

            if !all_types.is_empty() {
                mod_content.push_str("## Types\n");
                for (td, owner) in &all_types {
                    let safe_td_name = sanitize_name(&td.name);
                    let type_file_name = format!("{}_{}", mod_name, safe_td_name);
                    mod_content.push_str(&format!(
                        "- defines:: [[{}|{} {}]]\n",
                        type_file_name,
                        td.kind.as_str(),
                        td.name
                    ));

                    // ── Type file ─────────────────────────────────────────────
                    let type_content = render_type_file(td, owner, &mod_name, &type_file_name);
                    files.push((
                        PathBuf::from(format!("types/{}.md", type_file_name)),
                        type_content,
                    ));
                }
            }

            if !all_functions.is_empty() {
                mod_content.push_str("\n## Functions\n");
                for f in &all_functions {
                    let params = f
                        .params
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let ret = f.return_type.as_deref().unwrap_or("void");
                    mod_content.push_str(&format!("### {}\n", f.name));
                    mod_content.push_str(&format!("`{}({}) -> {}`\n", f.name, params, ret));

                    if !f.calls.is_empty() {
                        mod_content.push_str("\nCalls:\n");
                        for call in &f.calls {
                            if let Some(target_type) = &call.target_type {
                                mod_content.push_str(&format!(
                                    "- calls:: [[{}|{}::{}]]\n",
                                    sanitize_name(target_type),
                                    target_type,
                                    call.target_method
                                ));
                            } else {
                                mod_content.push_str(&format!(
                                    "- `{}` (local)\n",
                                    call.target_method
                                ));
                            }
                        }
                    }
                }
            }

            files.push((
                PathBuf::from(format!("modules/{}.md", mod_name)),
                mod_content,
            ));
        }

        files.push((PathBuf::from("Index.md"), index_content));

        // Generate topology canvas
        let canvas_json = super::canvas::generate_topology(project);
        files.push((PathBuf::from("Topology.canvas"), canvas_json));

        RenderOutput::Multiple(files)
    }
}

// ── Type file renderer ────────────────────────────────────────────────────────

fn render_type_file(
    td: &TypeDef,
    module: &Module,
    mod_name: &str,
    type_file_name: &str,
) -> String {
    let tag = td.kind.as_str().to_lowercase().replace(' ', "-");

    // ── YAML frontmatter (Dataview-compatible) ────────────────────────────────
    let mut content = String::from("---\n");
    content.push_str(&format!("tags:\n  - type\n  - {}\n", tag));
    content.push_str(&format!("kind: \"{}\"\n", td.kind.as_str()));
    content.push_str(&format!("name: \"{}\"\n", td.name));
    content.push_str(&format!("module: \"{}\"\n", module.path));
    content.push_str(&format!("language: \"{}\"\n", module.language.as_str()));
    if td.visibility != Visibility::Private {
        content.push_str(&format!("visibility: \"{}\"\n", td.visibility.as_str()));
    }
    if !td.type_params.is_empty() {
        content.push_str(&format!(
            "type-params: [{}]\n",
            td.type_params
                .iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !td.annotations.is_empty() {
        content.push_str("annotations:\n");
        for ann in &td.annotations {
            content.push_str(&format!("  - \"{}\"\n", ann.name));
        }
    }
    content.push_str("---\n\n");

    // ── Title ─────────────────────────────────────────────────────────────────
    let title = if td.type_params.is_empty() {
        format!("{} {}", td.kind.as_str(), td.name)
    } else {
        format!(
            "{} {}<{}>",
            td.kind.as_str(),
            td.name,
            td.type_params.join(", ")
        )
    };
    content.push_str(&format!("# {}\n\n", title));

    // ── Inline fields (Dataview + Juggl compatible) ───────────────────────────
    content.push_str(&format!(
        "- member-of:: [[{}|{} (module)]]\n",
        mod_name, module.path
    ));
    content.push_str(&format!("- kind:: {}\n", td.kind.as_str()));
    if td.visibility != Visibility::Private {
        content.push_str(&format!("- visibility:: {}\n", td.visibility.as_str()));
    }

    // ── Relations ─────────────────────────────────────────────────────────────
    if !td.relations.is_empty() {
        content.push_str("\n## Relations\n");
        for rel in &td.relations {
            let (rel_key, rel_label) = match rel.kind {
                RelationKind::Extends => ("extends", "extends"),
                RelationKind::Implements => ("implements", "implements"),
                RelationKind::ImplTrait => ("impl", "impl"),
            };
            let target_san = sanitize_name(&rel.target);
            content.push_str(&format!(
                "- {}:: [[{}|{}]]\n",
                rel_key, target_san, rel.target
            ));
            // Juggl edge label hint
            content.push_str(&format!(
                "  - edge-label:: \"{}\"\n",
                rel_label
            ));
        }
    }

    // ── Fields ────────────────────────────────────────────────────────────────
    if !td.fields.is_empty() {
        content.push_str("\n## Fields\n");
        content.push_str("| Name | Type | Visibility |\n");
        content.push_str("|------|------|------------|\n");
        for f in &td.fields {
            content.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                f.name,
                f.type_name,
                f.visibility.as_str()
            ));
        }
    }

    // ── Enum variants ─────────────────────────────────────────────────────────
    if !td.enum_variants.is_empty() {
        content.push_str("\n## Variants\n");
        for v in &td.enum_variants {
            content.push_str(&format!("- `{}`\n", v));
        }
    }

    // ── Methods ───────────────────────────────────────────────────────────────
    if !td.methods.is_empty() {
        content.push_str("\n## Methods\n");
        for m in &td.methods {
            let params = m
                .params
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let ret = m.return_type.as_deref().unwrap_or("void");
            let static_marker = if m.is_static { " `static`" } else { "" };

            // Annotations
            for ann in &m.annotations {
                content.push_str(&format!("  _{}_\n", ann.name));
            }

            content.push_str(&format!(
                "### `{}({})`{}\n",
                m.name, params, static_marker
            ));
            content.push_str(&format!("**Returns:** `{}`\n", ret));

            if !m.calls.is_empty() {
                // Deduplicate by target_type for clean Juggl graph edges
                let mut seen_types = std::collections::HashSet::new();
                content.push_str("\n**Calls:**\n");
                for call in &m.calls {
                    if let Some(target_type) = &call.target_type {
                        let target_san = sanitize_name(target_type);
                        content.push_str(&format!(
                            "- calls:: [[{}|{}::{} ]]\n",
                            target_san, target_type, call.target_method
                        ));
                        // One Juggl edge per unique target type
                        if seen_types.insert(target_san.clone()) {
                            content.push_str(&format!(
                                "  - edge-label:: \"calls\"\n"
                            ));
                        }
                    } else {
                        content.push_str(&format!(
                            "- `{}` (local call)\n",
                            call.target_method
                        ));
                    }
                }
            }

            if !m.callers.is_empty() {
                let mut seen_callers = std::collections::HashSet::new();
                content.push_str("\n**Called by:**\n");
                for caller in &m.callers {
                    let label = format!("{}", caller);
                    if seen_callers.insert(label.clone()) {
                        let link_target = caller.source_type
                            .as_deref()
                            .map(sanitize_name)
                            .unwrap_or_else(|| sanitize_name(&caller.source_method));
                        content.push_str(&format!(
                            "- called-by:: [[{}|{}]]\n",
                            link_target, label
                        ));
                    }
                }
            }

            content.push('\n');
        }
    }

    // ── Back-link to type file ────────────────────────────────────────────────
    content.push_str(&format!(
        "\n---\n*Generated by [skelecode](https://github.com/skelecode/skelecode) · [[{}]]*\n",
        type_file_name
    ));

    content
}

fn sanitize_name(name: &str) -> String {
    name.replace('/', "_")
        .replace('\\', "_")
        .replace("::", "_")
        .replace(':', "_")
        .replace('<', "_")
        .replace('>', "_")
        .replace(' ', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obsidian_export_on_self() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest_dir);
        let project = crate::scan_project(&path, &[Language::Rust], &[]);

        let renderer = ObsidianRenderer;
        match renderer.render(&project) {
            RenderOutput::Multiple(files) => {
                assert!(!files.is_empty(), "Vault should have been created");
                for (file_path, content) in &files {
                    let file_name = file_path.to_string_lossy();
                    assert!(!file_name.contains('<'), "Filename contains invalid character '<': {}", file_name);
                    assert!(!file_name.contains('>'), "Filename contains invalid character '>': {}", file_name);
                    assert!(!file_name.contains(':'), "Filename contains invalid character ':': {}", file_name);
                    assert!(!content.is_empty());
                }

                // Verify frontmatter exists in all markdown files
                for (path, content) in &files {
                    if path.extension().is_some_and(|e| e == "md") {
                        assert!(content.starts_with("---\n"), "All .md files should have YAML frontmatter");
                        assert!(content.contains("tags:"), "All .md files should have tags");
                    }
                }

                // Verify canvas file is generated
                let has_canvas = files.iter().any(|(p, _)| p.to_string_lossy().ends_with(".canvas"));
                assert!(has_canvas, "Vault should include Topology.canvas");

                // Verify inline fields are used for relationships
                let has_defines = files.iter().any(|(_, c)| c.contains("defines::"));
                let has_member_of = files.iter().any(|(_, c)| c.contains("member-of::"));
                assert!(has_defines, "Module files should use 'defines::' inline field");
                assert!(has_member_of, "Type files should use 'member-of::' inline field");

                // Verify new Dataview/Juggl fields
                let has_kind = files.iter().any(|(_, c)| c.contains("kind::"));
                assert!(has_kind, "Type files should have 'kind::' inline field");

                // Verify table format for fields
                let has_table = files.iter().any(|(_, c)| c.contains("| Name | Type |"));
                assert!(has_table, "Type files should use table format for fields");
            }
            _ => panic!("Expected multiple files format"),
        }
    }
}
