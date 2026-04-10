/// Unified intermediate representation for code structure.
/// Language-agnostic model that captures types, methods, fields,
/// call relationships, and type hierarchies.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Language {
    Rust,
    Java,
    JavaScript,
    Kotlin,
    Python,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Java => "java",
            Language::JavaScript => "js",
            Language::Kotlin => "kotlin",
            Language::Python => "python",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Struct,
    Enum,
    Trait,
    Class,
    Interface,
    Object,
    Record,
    DataClass,
    SealedClass,
}

impl TypeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TypeKind::Struct => "struct",
            TypeKind::Enum => "enum",
            TypeKind::Trait => "trait",
            TypeKind::Class => "class",
            TypeKind::Interface => "interface",
            TypeKind::Object => "object",
            TypeKind::Record => "record",
            TypeKind::DataClass => "data class",
            TypeKind::SealedClass => "sealed class",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
    Crate,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "pub",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Internal => "internal",
            Visibility::Crate => "pub(crate)",
        }
    }

    pub fn mermaid_marker(&self) -> &'static str {
        match self {
            Visibility::Public => "+",
            Visibility::Private => "-",
            Visibility::Protected => "#",
            Visibility::Internal | Visibility::Crate => "~",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationKind {
    Extends,
    Implements,
    ImplTrait,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRef {
    /// None for free functions or same-module calls
    pub target_type: Option<String>,
    pub target_method: String,
}

impl std::fmt::Display for CallRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.target_type {
            Some(t) => write!(f, "{}::{}", t, self.target_method),
            None => write!(f, "{}", self.target_method),
        }
    }
}

/// Reverse call reference: records which caller invokes a given method/function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerRef {
    /// `None` if the caller is a free function (not inside any type).
    pub source_type: Option<String>,
    pub source_method: String,
}

impl std::fmt::Display for CallerRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.source_type {
            Some(t) => write!(f, "{}::{}", t, self.source_method),
            None => write!(f, "{}", self.source_method),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRelation {
    pub kind: RelationKind,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub type_name: String,
}

impl std::fmt::Display for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() || self.name == "_" {
            write!(f, "{}", self.type_name)
        } else {
            write!(f, "{}:{}", self.name, self.type_name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub type_name: String,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub visibility: Visibility,
    pub calls: Vec<CallRef>,
    pub callers: Vec<CallerRef>,
    pub annotations: Vec<Annotation>,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub visibility: Visibility,
    pub calls: Vec<CallRef>,
    pub callers: Vec<CallerRef>,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub kind: TypeKind,
    pub visibility: Visibility,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub relations: Vec<TypeRelation>,
    pub annotations: Vec<Annotation>,
    pub type_params: Vec<String>,
    pub enum_variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub path: String,
    pub language: Language,
    pub types: Vec<TypeDef>,
    pub functions: Vec<Function>,
    /// Imported names parsed from `use`/`import` statements in this file.
    pub imports: Vec<ImportedName>,
}

/// A single imported name, produced by `use` / `import` statements.
///
/// `alias`     — the short identifier used in source code
/// `qualified` — canonical `"{module_path}::{TypeName}"` form;
///               `sanitize_name(qualified)` yields the Obsidian type-file stem
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedName {
    pub alias: String,
    pub qualified: String,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub modules: Vec<Module>,
}
