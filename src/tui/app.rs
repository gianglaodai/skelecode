use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::widgets::ListState;
use std::io;

use crate::ir::{Language, Project};

use super::export::{ExportApp, ExportFormat, ExportStatus};
use super::welcome::{LangOption, WelcomeApp, WelcomeConfig};


/// A node in the tree view.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub detail_machine: String,
    pub detail_obsidian: String,
    pub depth: u16,
    pub expanded: bool,
    pub has_children: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Machine,
    Obsidian,
}

pub struct App {
    pub nodes: Vec<TreeNode>,
    pub visible: Vec<usize>,
    pub selected: usize,
    pub list_state: ListState,
    pub tab: DetailTab,
    pub detail_scroll: u16,
    pub should_quit: bool,
    pub should_go_back: bool,
    pub project: Project,
    pub export_overlay: Option<ExportApp>,
    pub search_mode: bool,
    pub search_query: String,
}

impl App {
    pub fn new(project: Project) -> Self {
        let nodes = build_tree(&project);
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let mut app = App {
            nodes,
            visible: Vec::new(),
            selected: 0,
            list_state,
            tab: DetailTab::Machine,
            detail_scroll: 0,
            should_quit: false,
            should_go_back: false,
            project,
            export_overlay: None,
            search_mode: false,
            search_query: String::new(),
        };
        app.rebuild_visible();
        app
    }

    fn set_selected(&mut self, idx: usize) {
        self.selected = idx;
        self.list_state.select(Some(idx));
        self.detail_scroll = 0;
    }

    pub fn rebuild_visible(&mut self) {
        self.visible.clear();

        if self.search_query.is_empty() {
            // Normal mode: respect expand/collapse tree state
            for (i, _) in self.nodes.iter().enumerate() {
                if self.is_visible(i) {
                    self.visible.push(i);
                }
            }
        } else {
            // Search mode: show matching nodes plus their nearest depth-0 parent for context
            let q = self.search_query.to_lowercase();
            let n = self.nodes.len();

            // Pre-compute which nodes match
            let matches: Vec<bool> = self.nodes.iter()
                .map(|node| node.label.to_lowercase().contains(&q))
                .collect();

            let mut added = vec![false; n];

            for i in 0..n {
                if !matches[i] {
                    continue;
                }

                // Find and add the depth-0 ancestor (module header) if not already added
                if self.nodes[i].depth > 0 {
                    for j in (0..i).rev() {
                        if self.nodes[j].depth == 0 && !added[j] {
                            added[j] = true;
                            self.visible.push(j);
                            break;
                        }
                    }
                }

                if !added[i] {
                    added[i] = true;
                    self.visible.push(i);
                }
            }

            // Sort visible indices to maintain tree order
            self.visible.sort_unstable();
        }

        if self.selected >= self.visible.len() && !self.visible.is_empty() {
            let idx = self.visible.len() - 1;
            self.set_selected(idx);
        } else if self.visible.is_empty() {
            self.selected = 0;
            self.list_state.select(Some(0));
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
        // ── Search mode input ──────────────────────────────────────────────────
        if self.search_mode {
            match key {
                KeyCode::Esc => {
                    // Clear search and return to normal mode
                    self.search_mode = false;
                    self.search_query.clear();
                    self.rebuild_visible();
                }
                KeyCode::Enter => {
                    // Commit: keep filter active, exit typing mode
                    self.search_mode = false;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.rebuild_visible();
                    self.set_selected(0);
                }
                KeyCode::Down => {
                    if self.selected + 1 < self.visible.len() {
                        let idx = self.selected + 1;
                        self.set_selected(idx);
                    }
                }
                KeyCode::Up => {
                    if self.selected > 0 {
                        let idx = self.selected - 1;
                        self.set_selected(idx);
                    }
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.rebuild_visible();
                    self.set_selected(0);
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if !self.search_query.is_empty() {
                    // Esc while query is active (but not in typing mode) → clear filter
                    self.search_query.clear();
                    self.rebuild_visible();
                } else {
                    self.should_go_back = true;
                }
            }
            KeyCode::Backspace | KeyCode::Char('b') => {
                self.should_go_back = true;
            }
            // Enter search mode
            KeyCode::Char('/') => {
                self.search_mode = true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.visible.len() {
                    let idx = self.selected + 1;
                    self.set_selected(idx);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    let idx = self.selected - 1;
                    self.set_selected(idx);
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
                                    self.set_selected(pos);
                                }
                                break;
                            }
                        }
                    }
                }
            }
            KeyCode::Tab => {
                self.tab = match self.tab {
                    DetailTab::Machine => DetailTab::Obsidian,
                    DetailTab::Obsidian => DetailTab::Machine,
                };
                self.detail_scroll = 0;
            }
            // Scroll detail panel
            KeyCode::Char('d') | KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(10);
            }
            KeyCode::Char('u') | KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(10);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.set_selected(0);
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !self.visible.is_empty() {
                    let idx = self.visible.len() - 1;
                    self.set_selected(idx);
                }
            }
            // Copy current detail panel content to clipboard
            KeyCode::Char('y') => {
                if let Some(node) = self.selected_node() {
                    let content = match self.tab {
                        DetailTab::Machine => node.detail_machine.clone(),
                        DetailTab::Obsidian => node.detail_obsidian.clone(),
                    };
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(content);
                    }
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
    use crate::renderer::obsidian::{ObsidianRenderer, render_type_file_pub};

    let mut nodes = Vec::new();

    let _machine_full = MachineRenderer.render(project);

    for module in &project.modules {
        // Module node
        let mod_tag = match module.language {
            Language::Java | Language::Kotlin => "@pkg",
            Language::JavaScript => "@file",
            Language::Rust | Language::Python => "@mod",
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

        let mod_obsidian = {
            let single = Project {
                modules: vec![module.clone()],
            };
            match ObsidianRenderer.render(&single) {
                crate::renderer::RenderOutput::Multiple(files) => {
                    let mut preview = String::new();
                    for (path, content) in files {
                        preview.push_str(&format!("--- File: {} ---\n{}\n", path.display(), content));
                    }
                    preview
                }
                _ => String::new(),
            }
        };

        nodes.push(TreeNode {
            label: format!("{} {} [{}]", mod_tag, module.path, module.language.as_str()),
            detail_machine: mod_machine,
            detail_obsidian: mod_obsidian,
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
                        "  {}({}){}{}\n",
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

            let type_obsidian = render_type_file_pub(td, module);

            nodes.push(TreeNode {
                label: type_label,
                detail_machine: detail,
                detail_obsidian: type_obsidian,
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
                if !m.callers.is_empty() {
                    let callers: Vec<String> = m.callers.iter().map(|c| format!("{}", c)).collect();
                    method_detail.push_str(&format!("  @callers[{}]\n", callers.join(", ")));
                    method_detail.push_str("\nCalled by:\n");
                    for c in &m.callers {
                        method_detail.push_str(&format!("  ← {}\n", c));
                    }
                }

                let label = format!("{}({}){}{}", m.name, params.join(", "), ret, static_marker,);

                let mut method_obsidian = format!(
                    "---\ntags:\n  - method\nname: \"{}\"\ntype: \"{}\"\nmodule: \"{}\"\n---\n\n",
                    m.name, td.name, module.path
                );
                method_obsidian.push_str(&format!(
                    "### `{}({})`{}{}\n",
                    m.name,
                    params.join(", "),
                    if ret.is_empty() { String::new() } else { format!(" -> {}", m.return_type.as_deref().unwrap_or("")) },
                    static_marker
                ));
                if !m.calls.is_empty() {
                    method_obsidian.push_str("\n**Calls:**\n");
                    for c in &m.calls {
                        if let Some(target_type) = &c.target_type {
                            method_obsidian.push_str(&format!(
                                "- calls:: [[{}|{}::{} ]]\n",
                                target_type.replace("::", "_"),
                                target_type,
                                c.target_method
                            ));
                        } else {
                            method_obsidian.push_str(&format!("- `{}` (local)\n", c.target_method));
                        }
                    }
                }
                if !m.callers.is_empty() {
                    method_obsidian.push_str("\n**Called by:**\n");
                    for c in &m.callers {
                        let link = c.source_type.as_deref().unwrap_or(&c.source_method).replace("::", "_");
                        method_obsidian.push_str(&format!("- called-by:: [[{}|{}]]\n", link, c));
                    }
                }
                method_obsidian.push_str(&format!("\n- member-of:: [[{}|{} (type)]]\n", td.name, td.name));

                nodes.push(TreeNode {
                    label,
                    detail_machine: method_detail,
                    detail_obsidian: method_obsidian,
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
            if !f.callers.is_empty() {
                let callers: Vec<String> = f.callers.iter().map(|c| format!("{}", c)).collect();
                fn_detail.push_str(&format!("  @callers[{}]\n", callers.join(", ")));
                fn_detail.push_str("\nCalled by:\n");
                for c in &f.callers {
                    fn_detail.push_str(&format!("  ← {}\n", c));
                }
            }

            let label = format!("fn {}({}){}", f.name, params.join(", "), ret);

            let mut fn_obsidian = format!(
                "---\ntags:\n  - function\nname: \"{}\"\nmodule: \"{}\"\n---\n\n",
                f.name, module.path
            );
            fn_obsidian.push_str(&format!(
                "### `fn {}({}){}`\n",
                f.name,
                params.join(", "),
                if ret.is_empty() { String::new() } else { format!(" -> {}", f.return_type.as_deref().unwrap_or("")) }
            ));
            fn_obsidian.push_str(&format!("- member-of:: [[{}|{} (module)]]\n", module.path, module.path));
            if !f.calls.is_empty() {
                fn_obsidian.push_str("\n**Calls:**\n");
                for c in &f.calls {
                    if let Some(target_type) = &c.target_type {
                        fn_obsidian.push_str(&format!(
                            "- calls:: [[{}|{}::{} ]]\n",
                            target_type.replace("::", "_"),
                            target_type,
                            c.target_method
                        ));
                    } else {
                        fn_obsidian.push_str(&format!("- `{}` (local)\n", c.target_method));
                    }
                }
            }
            if !f.callers.is_empty() {
                fn_obsidian.push_str("\n**Called by:**\n");
                for c in &f.callers {
                    let link = c.source_type.as_deref().unwrap_or(&c.source_method).replace("::", "_");
                    fn_obsidian.push_str(&format!("- called-by:: [[{}|{}]]\n", link, c));
                }
            }

            nodes.push(TreeNode {
                label,
                detail_machine: fn_detail,
                detail_obsidian: fn_obsidian,
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
    let mut app = App::new(project);
    let result = run_app(&mut terminal, &mut app);
    ratatui::restore();
    result
}

/// Phase 1: welcome screen loop → Phase 2: main browse loop.
/// Loops back to the welcome screen if the user presses Back in the main view.
fn run_welcome_then_main(
    terminal: &mut DefaultTerminal,
    initial_welcome: WelcomeApp,
) -> io::Result<()> {
    let mut welcome = initial_welcome;

    loop {
        // ── Phase 1: welcome ──────────────────────────────────────────────────
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

        // ── Phase 2: scan & main view ─────────────────────────────────────────
        let config: WelcomeConfig = welcome.into_config();

        let languages: Vec<Language> = match config.language {
            LangOption::All => vec![],
            LangOption::Rust => vec![Language::Rust],
            LangOption::JavaBased => vec![Language::Java, Language::Kotlin],
            LangOption::JsTs => vec![Language::JavaScript],
            LangOption::Python => vec![Language::Python],
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
        let mut app = App::new(project);
        run_app(terminal, &mut app)?;

        if app.should_quit {
            return Ok(());
        }

        // app.should_go_back == true: loop back to a fresh welcome screen
        welcome = WelcomeApp::new();
    }
}

/// Main browse event loop.
fn run_app(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| {
            super::ui::draw(frame, app);
            if let Some(ref export) = app.export_overlay {
                super::ui::draw_export_overlay(frame, export);
            }
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if app.export_overlay.is_some() {
                handle_export_event(app, key.code);
            } else {
                app.handle_key(key.code);
            }
        }

        if app.should_quit || app.should_go_back {
            return Ok(());
        }
    }
}

/// Handle a key event when the export overlay is open.
fn handle_export_event(app: &mut App, key: KeyCode) {
    use crate::renderer::Renderer;
    use crate::renderer::machine::MachineRenderer;
    use crate::renderer::obsidian::ObsidianRenderer;
    use std::path::PathBuf;

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
                match MachineRenderer.render(&app.project) {
                    crate::renderer::RenderOutput::Single(content) => {
                        std::fs::write(&path, content)
                            .map(|_| format!("Saved to {}", path))
                            .map_err(|e| e.to_string())
                    }
                    _ => Err("Unexpected output format".into())
                }
            }
            ExportFormat::Vault => {
                match ObsidianRenderer.render(&app.project) {
                    crate::renderer::RenderOutput::Multiple(files) => {
                        let base_path = PathBuf::from(&path);
                        if let Err(e) = std::fs::create_dir_all(&base_path) {
                            Err(format!("Error creating directory: {}", e))
                        } else {
                            let _ = std::fs::create_dir_all(base_path.join("modules"));
                            let _ = std::fs::create_dir_all(base_path.join("types"));
                            let mut success = true;
                            let mut err_msg = String::new();
                            for (rel_path, content) in files {
                                let full_path = base_path.join(rel_path);
                                if let Err(e) = std::fs::write(&full_path, content) {
                                    success = false;
                                    err_msg = format!("Error writing {}: {}", full_path.display(), e);
                                    break;
                                }
                            }
                            if success {
                                Ok(format!("Saved Vault to {}", path))
                            } else {
                                Err(err_msg)
                            }
                        }
                    }
                    _ => Err("Unexpected output format".into())
                }
            }
            ExportFormat::Both => {
                Err("Both format is not supported for Vault yet. Please export separately.".into())
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
