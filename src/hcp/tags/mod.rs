//! Tag bindings module - shared models and API for workspace and project tag bindings

mod api;
mod commands;
mod models;

pub use api::{TagTarget, TagTargetKind};
pub use commands::{run_delete_tag_command, run_get_tag_command, run_set_tag_command};
pub use models::{OrgTag, OrgTagAttributes, TagBinding, TagBindingAttributes, TagBindingsResponse};
