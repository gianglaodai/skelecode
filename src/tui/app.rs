use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::io;

use crate::ir::{Language, Project};

use super::export::{ExportApp, ExportFormat, ExportStatus};
use super::welcome::{LangOption, WelcomeApp, WelcomeConfig};


/// A node in the tree view.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub detail: String,
    pub depth: u16,
    pub expanded: bool,
    pub has_children: bool,
    // Reserved for future use (child range tracking).
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Machine,
    Mermaid,
}

pub struct App {
    pub nodes: Vec<TreeNode>,
    pub visible: Vec<usize>,
    pub selected: usize,
    pub tab: DetailTab,
    pub should_quit: bool,
    pub project: Project,
    pub export_overlay: Option<ExportApp>,
}

impl App {
    pub fn new(project: Project) -> Self {
        let nodes = build_tree(&project);
        let mut app = App {
            nodes,
            visible: Vec::new(),
            selected: 0,
            tab: DetailTab::Machine,
            should_quit: false,
            project,
            export_overlay: None,
        };
        app.rebuild_visible();
        app
    }

    pub fn rebuild_visible(&mut self) {
        self.visible.clear();
        for (i, _node) in self.nodes.iter().enumerate() {
            if self.is_visible(i) {
                self.visible.push(i);
            }
        }
        if self.selected >= self.visible.len() && !self.visible.is_empty() {
            self.selected = self.visible.len() - 1;
        }
    }

    fn is_visible(&self, idx: usize) -> bool {
        let node = &self.nodes[idx];
        if node.depth == 0 {
            return true;
        }
        // Walk up to find parent, check if all ancestors are expanded
        for i in (0..idx).rev() {
            if self.nodes[i].depth < node.depth {
                if !self.nodes[i].expanded {
                    return false;
                }
                if self.nodes[i].depth == 0 {
                    return true;
                }
            }
        }
        true
    }

    pub fn selected_node(&self) -> Option<&TreeNode> {
        self.visible.get(self.selected).map(|&i| &self.nodes[i])
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.visible.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(&idx) = self.visible.get(self.selected)
                    && self.nodes[idx].has_children
                {
                    self.nodes[idx].expanded = !self.nodes[idx].expanded;
                    self.rebuild_visible();
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(&idx) = self.visible.get(self.selected) {
                    if self.nodes[idx].expanded {
                        self.nodes[idx].expanded = false;
                        self.rebuild_visible();
                    } else if self.nodes[idx].depth > 0 {
                        // Jump to parent
                        let depth = self.nodes[idx].depth;
                        for i in (0..idx).rev() {
                            if self.nodes[i].depth < depth {
                                if let Some(pos) = self.visible.iter().position(|&vi| vi == i) {
                                    self.selected = pos;
                                }
                                break;
                            }
                        }
                    }
                }
            }
            KeyCode::Tab => {
                self.tab = match self.tab {
                    DetailTab::Machine => DetailTab::Mermaid,
                    DetailTab::Mermaid => DetailTab::Machine,
                };
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !self.visible.is_empty() {
                    self.selected = self.visible.len() - 1;
                }
            }
            // Open export overlay
            KeyCode::Char('e') => {
                self.export_overlay = Some(ExportApp::new());
            }
            _ => {}
        }
    }
}

fn build_tree(project: &Project) -> Vec<TreeNode> {
    use crate::ir::*;
    use crate::renderer::Renderer;
    use crate::renderer::machine::MachineRenderer;
    use crate::renderer::mermaid::MermaidRenderer;

    let mut nodes = Vec::new();

    // Pre-render full outputs for detail tabs
    let _machine_full = MachineRenderer.render(project);
    let _mermaid_full = MermaidRenderer.render(project);

    for module in &project.modules {
        // Module node
        let mod_tag = match module.language {
            Language::Java | Language::Kotlin => "@pkg",
            Language::JavaScript => "@file",
            Language::Rust => "@mod",
        };

        // Build module-level machine context for detail
        let mut mod_machine = format!(
            "@lang {}\n{} {}\n",
            module.language.as_str(),
            mod_tag,
            module.path
        );
        for td in &module.types {
            mod_machine.push_str(&format!("  @type {} [{}]\n", td.name, td.kind.as_str()));
        }
        for f in &module.functions {
            mod_machine.push_str(&format!("  @fn {}\n", f.name));
        }

        let mod_mermaid = {
            let single = Project {
                modules: vec![module.clone()],
            };
            let raw = MermaidRenderer.render(&single);
            // Strip markdown fences and module header for TUI display
            raw.lines()
                .filter(|l| {
                    !l.starts_with("## Module:")
                        && !l.starts_with("```")
                        && !l.is_empty()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        nodes.push(TreeNode {
            label: format!("{} {} [{}]", mod_tag, module.path, module.language.as_str()),
            detail: format!(
                "--- Machine Context ---\n{}\n--- Mermaid ---\n{}",
                mod_machine, mod_mermaid
            ),
            depth: 0,
            expanded: false,
            has_children: !module.types.is_empty() || !module.functions.is_empty(),
        });

        // Types
        for td in &module.types {
            let fields_str: Vec<String> = td
                .fields
                .iter()
                .map(|f| format!("{}:{}", f.name, f.type_name))
                .collect();
            let fields_inline = if fields_str.is_empty() {
                String::new()
            } else {
                format!(" {{{}}}", fields_str.join(", "))
            };

            let mut detail = String::new();
            detail.push_str(&format!(
                "@type {} [{}]{}\n",
                td.name,
                td.kind.as_str(),
                fields_inline
            ));
            if td.visibility != Visibility::Private {
                detail.push_str(&format!("  @vis {}\n", td.visibility.as_str()));
            }
            if !td.type_params.is_empty() {
                detail.push_str(&format!("  @gen <{}>\n", td.type_params.join(", ")));
            }
            if !td.enum_variants.is_empty() {
                detail.push_str(&format!("  @enum {}\n", td.enum_variants.join(", ")));
            }
            for rel in &td.relations {
                match rel.kind {
                    RelationKind::Extends => detail.push_str(&format!("  @ext {}\n", rel.target)),
                    RelationKind::Implements | RelationKind::ImplTrait => {
                        detail.push_str(&format!("  @impl {}\n", rel.target))
                    }
                }
            }
            for ann in &td.annotations {
                detail.push_str(&format!("  @ann {}\n", ann.name));
            }

            // Fields detail
            if !td.fields.is_empty() {
                detail.push_str("\nFields:\n");
                for f in &td.fields {
                    detail.push_str(&format!(
                        "  {} {} : {}\n",
                        f.visibility.as_str(),
                        f.name,
                        f.type_name
                    ));
                }
            }

            // Methods summary
            if !td.methods.is_empty() {
                detail.push_str(&format!("\nMethods ({}):\n", td.methods.len()));
                for m in &td.methods {
                    let params: Vec<String> = m.params.iter().map(|p| format!("{}", p)).collect();
                    let ret = m
                        .return_type
                        .as_ref()
                        .map(|r| format!("->{}", r))
                        .unwrap_or_default();
                    let static_marker = if m.is_static { " [static]" } else { "" };
                    detail.push_str(&format!(
                        "  {}({}){}{}\\n",
                        m.name,
                        params.join(", "),
                        ret,
                        static_marker,
                    ));
                }
            }

            let type_label = format!(
                "{} [{}]{}",
                td.name,
                td.kind.as_str(),
                if !td.type_params.is_empty() {
                    format!("<{}>", td.type_params.join(", "))
                } else {
                    String::new()
                }
            );

            nodes.push(TreeNode {
                label: type_label,
                detail,
                depth: 1,
                expanded: false,
                has_children: !td.methods.is_empty(),
            });

            // Methods as children of type
            for m in &td.methods {
                let params: Vec<String> = m.params.iter().map(|p| format!("{}", p)).collect();
                let ret = m
                    .return_type
                    .as_ref()
                    .map(|r| format!("->{}", r))
                    .unwrap_or_default();
                let static_marker = if m.is_static { " @static" } else { "" };

                let mut method_detail = format!(
                    "@fn {}({}){} @vis {}{}\n",
                    m.name,
                    params.join(", "),
                    ret,
                    m.visibility.as_str(),
                    static_marker,
                );

                if !m.calls.is_empty() {
                    let calls: Vec<String> = m.calls.iter().map(|c| format!("{}", c)).collect();
                    method_detail.push_str(&format!("  @calls[{}]\n", calls.join(", ")));
                    method_detail.push_str("\nCall graph:\n");
                    for c in &m.calls {
                        method_detail.push_str(&format!("  → {}\n", c));
                    }
                }

                let label = format!("{}({}){}{}", m.name, params.join(", "), ret, static_marker,);

                nodes.push(TreeNode {
                    label,
                    detail: method_detail,
                    depth: 2,
                    expanded: false,
                    has_children: false,
                });
            }
        }

        // Free functions
        for f in &module.functions {
            let params: Vec<String> = f.params.iter().map(|p| format!("{}", p)).collect();
            let ret = f
                .return_type
                .as_ref()
                .map(|r| format!("->{}", r))
                .unwrap_or_default();

            let mut fn_detail = format!(
                "@fn {}({}){}  @vis {}\n",
                f.name,
                params.join(", "),
                ret,
                f.visibility.as_str(),
            );

            if !f.calls.is_empty() {
                let calls: Vec<String> = f.calls.iter().map(|c| format!("{}", c)).collect();
                fn_detail.push_str(&format!("  @calls[{}]\n", calls.join(", ")));
                fn_detail.push_str("\nCall graph:\n");
                for c in &f.calls {
                    fn_detail.push_str(&format!("  → {}\n", c));
                }
            }

            let label = format!("fn {}({}){}", f.name, params.join(", "), ret);

            nodes.push(TreeNode {
                label,
                detail: fn_detail,
                depth: 1,
                expanded: false,
                has_children: false,
            });
        }
    }

    nodes
}

// ─── TUI entry points ─────────────────────────────────────────────────────────

/// Run the TUI starting from the welcome/input screen.
/// Called when no path is given on the command line.
pub fn run_tui_welcome() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_welcome_then_main(&mut terminal, WelcomeApp::new());
    ratatui::restore();
    result
}

/// Run the TUI directly into the main browse view.
/// Called when a path is already given on the command line.
pub fn run_tui(project: Project) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, App::new(project));
    ratatui::restore();
    result
}

/// Phase 1: welcome screen loop → Phase 2: main browse loop.
fn run_welcome_then_main(
    terminal: &mut DefaultTerminal,
    mut welcome: WelcomeApp,
) -> io::Result<()> {
    // ── Phase 1: welcome ──────────────────────────────────────────────────────
    loop {
        terminal.draw(|frame| super::ui::draw_welcome(frame, &welcome))?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            welcome.handle_key(key.code, key.modifiers);
        }

        if welcome.should_quit {
            return Ok(());
        }

        if welcome.confirmed {
            break;
        }
    }

    // ── Phase 2: scan & main view ─────────────────────────────────────────────
    let config: WelcomeConfig = welcome.into_config();

    let languages: Vec<Language> = match config.language {
        LangOption::All => vec![],
        LangOption::Rust => vec![Language::Rust],
        LangOption::Java => vec![Language::Java],
        LangOption::JavaScript => vec![Language::JavaScript],
        LangOption::Kotlin => vec![Language::Kotlin],
    };

    // Show a brief "Scanning…" message while we parse
    terminal.draw(|frame| {
        let area = frame.area();
        let msg = Paragraph::new("  Scanning project, please wait…")
            .style(
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Cyan)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .block(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .title(" skelecode ")
                    .title_alignment(ratatui::layout::Alignment::Center)
                    .border_style(
                        ratatui::style::Style::default().fg(ratatui::style::Color::Cyan),
                    ),
            );
        frame.render_widget(msg, area);
    })?;

    let project = crate::scan_project(&config.path, &languages, &config.exclude_patterns);
    run_app(terminal, App::new(project))
}

/// Main browse event loop.
fn run_app(terminal: &mut DefaultTerminal, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| {
            super::ui::draw(frame, &app);
            if let Some(ref export) = app.export_overlay {
                super::ui::draw_export_overlay(frame, export);
            }
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if app.export_overlay.is_some() {
                handle_export_event(&mut app, key.code);
            } else {
                app.handle_key(key.code);
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

/// Handle a key event when the export overlay is open.
fn handle_export_event(app: &mut App, key: KeyCode) {
    use crate::renderer::Renderer;
    use crate::renderer::machine::MachineRenderer;
    use crate::renderer::mermaid::MermaidRenderer;

    let Some(ref mut export) = app.export_overlay else { return };

    export.handle_key(key);

    if export.should_close {
        app.export_overlay = None;
        return;
    }

    if export.do_export {
        export.do_export = false;
        let path = export.path_input.trim().to_string();
        if path.is_empty() {
            export.status = Some(ExportStatus::Error("Output path cannot be empty".into()));
            return;
        }

        let format = export.selected_format();
        let result = match format {
            ExportFormat::Machine => {
                let content = MachineRenderer.render(&app.project);
                std::fs::write(&path, content)
                    .map(|_| format!("Saved to {}", path))
                    .map_err(|e| e.to_string())
            }
            ExportFormat::Mermaid => {
                let content = MermaidRenderer.render(&app.project);
                std::fs::write(&path, content)
                    .map(|_| format!("Saved to {}", path))
                    .map_err(|e| e.to_string())
            }
            ExportFormat::Both => {
                let machine = MachineRenderer.render(&app.project);
                let mermaid = MermaidRenderer.render(&app.project);
                let combined =
                    format!("# Machine Context\n\n{}\n\n---\n\n# Mermaid\n\n{}", machine, mermaid);
                std::fs::write(&path, combined)
                    .map(|_| format!("Saved to {}", path))
                    .map_err(|e| e.to_string())
            }
        };

        export.status = Some(match result {
            Ok(msg) => ExportStatus::Success(msg),
            Err(e) => ExportStatus::Error(e),
        });
    }
}

// Paragraph used for the scanning message in run_welcome_then_main
use ratatui::widgets::Paragraph;
