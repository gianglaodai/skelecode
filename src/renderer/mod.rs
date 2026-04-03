pub mod machine;
pub mod mermaid;

use crate::ir::Project;

pub trait Renderer {
    fn render(&self, project: &Project) -> String;
}
