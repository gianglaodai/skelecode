use std::collections::{HashMap, HashSet};
use crate::ir::*;

/// Generate an Obsidian Canvas (.canvas) JSON topology from a project IR.
///
/// Layout: modules arranged in a grid (max 4 columns), each module is a group
/// containing its type nodes. Edges are labeled with relationship types.
///
/// Color legend (Obsidian preset):
///   "1" red    = class/object nodes
///   "2" orange = enum nodes + "calls" edges
///   "3" yellow = free-function nodes
///   "4" green  = struct nodes + "defines" edges
///   "5" cyan   = module nodes
///   "6" purple = trait/interface nodes + "extends"/"implements" edges
pub fn generate_topology(project: &Project) -> String {
    let mut nodes: Vec<String> = Vec::new();
    let mut edges: Vec<String> = Vec::new();
    let mut nid: usize = 0;
    let mut eid: usize = 0;

    // Lookup tables: type-name → canvas node id
    let mut type_node: HashMap<String, String> = HashMap::new();
    let mut mod_node: HashMap<String, String> = HashMap::new();

    // ── Layout constants ───────────────────────────────────────────────────
    let cols_wrap = 4i32;
    let col_w = 520;
    let row_h = 80;
    let mod_w = 340;
    let mod_h = 50;
    let type_w = 280;
    let type_h = 44;
    let fn_w = 280;
    let fn_h = 44;
    let pad_x = 20; // indent types inside group
    let group_pad = 20;

    // Pre-compute the max vertical extent per grid-row so rows don't overlap.
    let grid_rows: Vec<Vec<usize>> = project
        .modules
        .iter()
        .enumerate()
        .fold(Vec::new(), |mut acc, (i, _)| {
            let r = i / cols_wrap as usize;
            if r >= acc.len() {
                acc.push(Vec::new());
            }
            acc[r].push(i);
            acc
        });

    let mut grid_row_y: Vec<i32> = Vec::new();
    let mut cum_y = 0i32;
    for row_indices in &grid_rows {
        grid_row_y.push(cum_y);
        let tallest = row_indices
            .iter()
            .map(|&i| {
                let m = &project.modules[i];
                let items = m.types.len() + if m.functions.is_empty() { 0 } else { 1 };
                items.max(1)
            })
            .max()
            .unwrap_or(1);
        // group header + items + bottom padding
        cum_y += (tallest as i32 + 2) * row_h + group_pad * 2;
    }

    // ── Phase 1: create nodes ──────────────────────────────────────────────
    for (i, module) in project.modules.iter().enumerate() {
        let gc = (i % cols_wrap as usize) as i32;
        let gr = i / cols_wrap as usize;
        let base_x = gc * col_w;
        let base_y = grid_row_y[gr];

        let mid = format!("m{}", nid);
        nid += 1;
        mod_node.insert(module.path.clone(), mid.clone());

        // Module text node
        nodes.push(format!(
            concat!(
                "{{",
                r#""id":"{}","type":"text","#,
                r#""text":"{}","#,
                r#""x":{},"y":{},"width":{},"height":{},"color":"5""#,
                "}}"
            ),
            mid,
            json_escape(&format!("**{}**\\n_{}_", module.path, module.language.as_str())),
            base_x + pad_x,
            base_y + group_pad,
            mod_w - pad_x * 2,
            mod_h,
        ));

        let mut slot = 1i32; // vertical slot (0 = module header)

        // Type nodes
        for td in &module.types {
            let tid = format!("t{}", nid);
            nid += 1;

            let color = type_color(&td.kind);
            let label = json_escape(&format!("**{}** {}", td.kind.as_str(), td.name));

            let tx = base_x + pad_x;
            let ty = base_y + group_pad + slot * row_h;
            slot += 1;

            nodes.push(format!(
                concat!(
                    "{{",
                    r#""id":"{}","type":"text","#,
                    r#""text":"{}","#,
                    r#""x":{},"y":{},"width":{},"height":{},"color":"{}""#,
                    "}}"
                ),
                tid, label, tx, ty, type_w, type_h, color,
            ));

            type_node.insert(td.name.clone(), tid.clone());

            // Edge: module → type ("defines")
            edges.push(make_edge(&mut eid, &mid, &tid, "bottom", "top", "defines", "4"));
        }

        // Free functions → single summary node per module
        if !module.functions.is_empty() {
            let fid = format!("f{}", nid);
            nid += 1;

            let fn_names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
            let summary = if fn_names.len() <= 6 {
                fn_names.join("\\n")
            } else {
                let mut s = fn_names[..5].join("\\n");
                s.push_str(&format!("\\n_+{} more_", fn_names.len() - 5));
                s
            };
            let label = json_escape(&format!("**functions**\\n{}", summary));

            let tx = base_x + pad_x;
            let ty = base_y + group_pad + slot * row_h;
            slot += 1;

            nodes.push(format!(
                concat!(
                    "{{",
                    r#""id":"{}","type":"text","#,
                    r#""text":"{}","#,
                    r#""x":{},"y":{},"width":{},"height":{},"color":"3""#,
                    "}}"
                ),
                fid, label, tx, ty, fn_w, fn_h,
            ));

            edges.push(make_edge(&mut eid, &mid, &fid, "bottom", "top", "contains", "3"));

            // Collect calls from free functions → type nodes
            let mut fn_call_targets: HashSet<String> = HashSet::new();
            for func in &module.functions {
                for call in &func.calls {
                    if let Some(ref tt) = call.target_type {
                        if let Some(target_id) = type_node.get(tt.as_str()) {
                            fn_call_targets.insert(target_id.clone());
                        }
                    }
                }
            }
            for target_id in fn_call_targets {
                edges.push(make_edge(&mut eid, &fid, &target_id, "right", "left", "calls", "2"));
            }
        }

        // Group node wrapping entire module
        let group_h = (slot + 1) * row_h;
        let gid = format!("g{}", nid);
        nid += 1;
        // Group nodes are rendered below other nodes, so insert at the beginning.
        nodes.insert(
            nodes.len() - (module.types.len() + if module.functions.is_empty() { 1 } else { 2 }),
            format!(
                concat!(
                    "{{",
                    r#""id":"{}","type":"group","#,
                    r#""label":"{}","#,
                    r#""x":{},"y":{},"width":{},"height":{},"color":"5""#,
                    "}}"
                ),
                gid,
                json_escape(&module.path),
                base_x,
                base_y,
                mod_w,
                group_h,
            ),
        );
    }

    // ── Phase 2: cross-type relation & call edges ──────────────────────────
    for module in &project.modules {
        for td in &module.types {
            let src = match type_node.get(&td.name) {
                Some(id) => id,
                None => continue,
            };

            // Inheritance / trait relations
            for rel in &td.relations {
                if let Some(tgt) = type_node.get(&rel.target) {
                    let (label, color) = match rel.kind {
                        RelationKind::Extends => ("extends", "1"),
                        RelationKind::Implements => ("implements", "6"),
                        RelationKind::ImplTrait => ("impl", "6"),
                    };
                    edges.push(make_edge(&mut eid, src, tgt, "right", "left", label, color));
                }
            }

            // Aggregate call targets from all methods (deduplicated per target type)
            let mut call_targets: HashSet<String> = HashSet::new();
            for method in &td.methods {
                for call in &method.calls {
                    if let Some(ref tt) = call.target_type {
                        if let Some(tgt) = type_node.get(tt.as_str()) {
                            if tgt != src {
                                call_targets.insert(tgt.clone());
                            }
                        }
                    }
                }
            }
            for tgt in call_targets {
                edges.push(make_edge(&mut eid, src, &tgt, "right", "left", "calls", "2"));
            }
        }
    }

    // ── Build final JSON ───────────────────────────────────────────────────
    let mut out = String::from("{\n  \"nodes\":[\n");
    for (i, n) in nodes.iter().enumerate() {
        out.push_str("    ");
        out.push_str(n);
        if i + 1 < nodes.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n  \"edges\":[\n");
    for (i, e) in edges.iter().enumerate() {
        out.push_str("    ");
        out.push_str(e);
        if i + 1 < edges.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}");
    out
}

fn make_edge(
    eid: &mut usize,
    from: &str,
    to: &str,
    from_side: &str,
    to_side: &str,
    label: &str,
    color: &str,
) -> String {
    let id = format!("e{}", eid);
    *eid += 1;
    format!(
        concat!(
            "{{",
            r#""id":"{}","fromNode":"{}","toNode":"{}","#,
            r#""fromSide":"{}","toSide":"{}","#,
            r#""label":"{}","color":"{}""#,
            "}}"
        ),
        id, from, to, from_side, to_side, label, color,
    )
}

fn type_color(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Struct | TypeKind::Record | TypeKind::DataClass => "4",   // green
        TypeKind::Enum | TypeKind::SealedClass => "2",                     // orange
        TypeKind::Trait | TypeKind::Interface => "6",                      // purple
        TypeKind::Class | TypeKind::Object => "1",                         // red
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_topology_on_self() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = std::path::PathBuf::from(manifest_dir);
        let project = crate::scan_project(&path, &[Language::Rust], &[]);

        let canvas = generate_topology(&project);

        // Must be valid-ish JSON (starts/ends correctly)
        assert!(canvas.starts_with('{'), "Canvas should be JSON object");
        assert!(canvas.ends_with('}'), "Canvas should end with }}");
        assert!(canvas.contains("\"nodes\""), "Must have nodes array");
        assert!(canvas.contains("\"edges\""), "Must have edges array");

        // Should contain known types
        assert!(canvas.contains("Project"), "Should have Project type");
        assert!(canvas.contains("Module"), "Should have Module type");

        // Should contain labeled edges
        assert!(canvas.contains("\"label\":\"defines\""), "Should have 'defines' edges");
    }
}
