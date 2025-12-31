//! Project module

mod api;
mod commands;
mod models;

pub use commands::run_prj_command;
pub use models::{Project, ProjectAttributes, ProjectWorkspaces, ProjectsResponse};
