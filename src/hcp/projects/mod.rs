//! Project module

mod api;
mod commands;
mod models;
pub mod resolver;

pub use commands::run_prj_command;
pub use models::{Project, ProjectAttributes, ProjectWorkspaces};
pub use resolver::{resolve_project, ResolvedProject};
