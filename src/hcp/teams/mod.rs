//! Teams module - list and get teams in organizations

mod api;
mod commands;
mod models;

pub use commands::run_team_command;
pub use models::{Team, TeamAttributes};
