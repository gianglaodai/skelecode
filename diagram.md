## Module: ir

```mermaid
classDiagram
    class Language {
        <<enum>>
        Rust
        Java
        JavaScript
        Kotlin
        +as_str() &
    }
    class TypeKind {
        <<enum>>
        Struct
        Enum
        Trait
        Class
        Interface
        Object
        Record
        DataClass
        SealedClass
        +as_str() &
    }
    class Visibility {
        <<enum>>
        Public
        Private
        Protected
        Internal
        Crate
        +as_str() &
        +mermaid_marker() &
    }
    class RelationKind {
        <<enum>>
        Extends
        Implements
        ImplTrait
    }
    class CallRef {
        +Option~String~ target_type
        +String target_method
        -fmt(&mut std::fmt::Formatter f) std::fmt
    }
    class TypeRelation {
        +RelationKind kind
        +String target
    }
    class Annotation {
        +String name
    }
    class Param {
        +String name
        +String type_name
        -fmt(&mut std::fmt::Formatter f) std::fmt
    }
    class Field {
        +String name
        +String type_name
        +Visibility visibility
    }
    class Method {
        +String name
        +Vec~Param~ params
        +Option~String~ return_type
        +Visibility visibility
        +Vec~CallRef~ calls
        +Vec~Annotation~ annotations
        +bool is_static
    }
    class Function {
        +String name
        +Vec~Param~ params
        +Option~String~ return_type
        +Visibility visibility
        +Vec~CallRef~ calls
    }
    class TypeDef {
        +String name
        +TypeKind kind
        +Visibility visibility
        +Vec~Field~ fields
        +Vec~Method~ methods
        +Vec~TypeRelation~ relations
        +Vec~Annotation~ annotations
        +Vec~String~ type_params
        +Vec~String~ enum_variants
    }
    class Module {
        +String path
        +Language language
        +Vec~TypeDef~ types
        +Vec~Function~ functions
    }
    class Project {
        +Vec~Module~ modules
    }
    CallRef ..|> std_fmt_Display : implements
    Param ..|> std_fmt_Display : implements
```

## Module: crate

```mermaid
classDiagram
    class OutputFormat {
        <<enum>>
        Mermaid
        Machine
        Both
    }
    class LangFilter {
        <<enum>>
        Rust
        Java
        Js
        Kotlin
        -to_language() Language
    }
    class Cli {
        -Option~PathBuf~ path
        -Option~OutputFormat~ format
        -Option~PathBuf~ output
        -Option~PathBuf~ output_mermaid
        -Option~PathBuf~ output_machine
        -Vec~LangFilter~ lang
        -Vec~String~ exclude
        -bool tui
        -bool verbose
    }
    class crate__functions {
        <<module>>
        +scan_project(&Path root, &[Language] languages, &[String] exclude_patterns) Project
        -create_parsers(&[Language] languages) Vec
        -is_hidden(&walkdir::DirEntry entry) bool
        -is_excluded(&walkdir::DirEntry entry, &[String] patterns) bool
        -main()
        -validate_path(&PathBuf path)
        -print_scan_stats(&skelecode::ir::Project project)
        -write_file(&PathBuf path, &str content)
    }
```

## Module: parser

```mermaid
classDiagram
    class LanguageParser {
        <<trait>>
        -language() Language
        -can_parse(&Path path) bool
        -parse_file(&Path path, &str source) Option
    }
    class parser__functions {
        <<module>>
        +detect_language(&Path path) Option
    }
```

## Module: parser::rust

```mermaid
classDiagram
    class RustParser {
        -std::cell::RefCell~Parser~ parser
        -default() Self$
        +new() Self$
        -language() Language
        -can_parse(&Path path) bool
        -parse_file(&Path path, &str source) Option
    }
    class ImplBlock {
        -String type_name
        -Option~TypeRelation~ trait_relation
        -Vec~Method~ methods
    }
    class parser_rust__functions {
        <<module>>
        -module_path_from_file(&Path path) String
        -node_text(Node node, & str source) &
        -extract_visibility(Node node, &str source) Visibility
        -extract_type_params(Node node, &str source) Vec
        -collect_type_params(Node node, &str source, &mut Vec~String~ params)
        -parse_struct(Node node, &str source) Option
        -parse_field(Node node, &str source) Option
        -parse_enum(Node node, &str source) Option
        -parse_trait(Node node, &str source) Option
        -parse_impl_block(Node node, &str source) Option
        -parse_method(Node node, &str source, bool _in_impl) Option
        -parse_free_function(Node node, &str source) Option
        -parse_parameters(Node node, &str source) (
        -parse_return_type(Node node, &str source) Option
        -extract_calls(Node node, &str source) Vec
        -resolve_receiver(Node node, &str source) String
        -collect_calls(Node node, &str source, &mut Vec~CallRef~ calls)
    }
    RustParser ..|> Default : implements
    RustParser ..|> LanguageParser : implements
    RustParser --> Self : uses
    RustParser --> Parser : uses
    RustParser --> Vec : uses
```

## Module: renderer::machine

```mermaid
classDiagram
    class MachineRenderer {
        -render(&Project project) String
    }
    class renderer_machine__functions {
        <<module>>
        -render_module(&Module module, &mut String out)
        -render_type(&TypeDef td, &mut String out)
        -render_method(&Method method, &mut String out)
        -render_function(&Function func, &mut String out)
    }
    MachineRenderer ..|> Renderer : implements
    MachineRenderer --> String : uses
```

## Module: renderer::mermaid

```mermaid
classDiagram
    class MermaidRenderer {
        -render(&Project project) String
    }
    class renderer_mermaid__functions {
        <<module>>
        -render_module_group(&str path, &[&Module] modules, &mut String out)
        -render_type(&TypeDef td, &mut String out)
        -render_free_function(&Function func, &mut String out)
        -render_relationships(&TypeDef td, &mut String out)
        -render_call_edges(&Module module, &mut String out)
        -is_valid_type_ref(&str name) bool
        -sanitize_id(&str name) String
        -sanitize_mermaid(&str text) String
        -strip_lifetimes(&str text) String
        -convert_generics(&str text) String
    }
    MermaidRenderer ..|> Renderer : implements
    MermaidRenderer --> String : uses
    MermaidRenderer --> Vec : uses
```

## Module: renderer

```mermaid
classDiagram
    class Renderer {
        <<trait>>
        -render(&Project project) String
    }
```

## Module: tui::app

```mermaid
classDiagram
    class TreeNode {
        +String label
        +String detail
        +u16 depth
        +bool expanded
        +bool has_children
    }
    class DetailTab {
        <<enum>>
        Machine
        Mermaid
    }
    class App {
        +Vec~TreeNode~ nodes
        +Vec~usize~ visible
        +usize selected
        +DetailTab tab
        +bool should_quit
        +Project project
        +Option~ExportApp~ export_overlay
        +new(Project project) Self$
        +rebuild_visible()
        -is_visible(usize idx) bool
        +selected_node() Option
        +handle_key(KeyCode key)
    }
    class tui_app__functions {
        <<module>>
        -build_tree(&Project project) Vec
        +run_tui_welcome() io::Result
        +run_tui(Project project) io::Result
        -run_welcome_then_main(&mut DefaultTerminal terminal, WelcomeApp welcome) io::Result
        -run_app(&mut DefaultTerminal terminal, App app) io::Result
        -handle_export_event(&mut App app, KeyCode key)
    }
    App --> Vec : uses
    App --> ExportApp : uses
```

## Module: tui::export

```mermaid
classDiagram
    class ExportFormat {
        <<enum>>
        Machine
        Mermaid
        Both
        +label() &
        +default_filename() &
    }
    class ExportField {
        <<enum>>
        FormatSelector
        PathInput
        ExportButton
        +next() Self
        +prev() Self
    }
    class ExportStatus {
        <<enum>>
        Success(String)
        Error(String)
    }
    class ExportApp {
        +usize format_index
        +String path_input
        +ExportField focused
        +Option~ExportStatus~ status
        +bool should_close
        +bool do_export
        +new() Self$
        +selected_format() ExportFormat
        +handle_key(KeyCode key)
        -auto_update_path()
    }
    ExportApp --> ExportFormat_Machine : uses
    ExportApp --> ExportFormat_ALL : uses
```

## Module: tui::ui

```mermaid
classDiagram
    class tui_ui__functions {
        <<module>>
        +draw(&mut Frame frame, &App app)
        -draw_tree(&mut Frame frame, &App app, Rect area)
        -draw_detail(&mut Frame frame, &App app, Rect area)
        -highlight_machine_line(&str line) Line
        -highlight_tags(&str text, &mut Vec~Span~ spans, Style tag_style, Style value_style)
        -draw_help(&mut Frame frame, &App app, Rect area)
        +draw_welcome(&mut Frame frame, &WelcomeApp app)
        -draw_banner(&mut Frame frame, Rect area)
        -draw_subtitle(&mut Frame frame, Rect area)
        -draw_text_input(&mut Frame frame, &str label, &str value, bool focused, Rect area)
        -draw_path_field(&mut Frame frame, &WelcomeApp app, Rect area)
        -draw_exclude_field(&mut Frame frame, &WelcomeApp app, Rect area)
        -draw_lang_selector(&mut Frame frame, &WelcomeApp app, Rect area)
        -draw_error(&mut Frame frame, &WelcomeApp app, Rect area)
        -draw_confirm_button(&mut Frame frame, &WelcomeApp app, Rect area)
        -draw_welcome_help(&mut Frame frame, Rect area)
        -centered_rect(u16 percent_x, u16 height, Rect r) Rect
        +draw_export_overlay(&mut Frame frame, &ExportApp export)
        -draw_export_format(&mut Frame frame, &ExportApp export, Rect area)
        -draw_export_path(&mut Frame frame, &ExportApp export, Rect area)
        -draw_export_status(&mut Frame frame, &ExportApp export, Rect area)
        -draw_export_button(&mut Frame frame, &ExportApp export, Rect area)
        -draw_export_help(&mut Frame frame, Rect area)
    }
```

## Module: tui::welcome

```mermaid
classDiagram
    class FocusedField {
        <<enum>>
        PathInput
        LangSelector
        ExcludeInput
        ConfirmButton
        +next() Self
        +prev() Self
    }
    class LangOption {
        <<enum>>
        All
        Rust
        Java
        JavaScript
        Kotlin
        +label() &
    }
    class WelcomeConfig {
        +PathBuf path
        +LangOption language
        +Vec~String~ exclude_patterns
    }
    class WelcomeApp {
        +String path_input
        +usize lang_index
        +String exclude_input
        +FocusedField focused
        +bool confirmed
        +bool should_quit
        +Option~String~ error_msg
        +new() Self$
        +selected_lang() LangOption
        +handle_key(KeyCode key, KeyModifiers _modifiers)
        -try_confirm()
        +into_config() WelcomeConfig
    }
    WelcomeApp --> String : uses
    WelcomeApp --> LangOption_ALL_OPTIONS : uses
    WelcomeApp --> PathBuf : uses
```
