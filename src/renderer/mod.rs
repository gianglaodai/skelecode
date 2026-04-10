pub mod canvas;
pub mod machine;
pub mod obsidian;

use crate::ir::Project;
use std::path::PathBuf;

pub enum RenderOutput {
    Single(String),
    Multiple(Vec<(PathBuf, String)>),
}

pub trait Renderer {
    fn render(&self, project: &Project) -> RenderOutput;
}
