//! Context management module
//!
//! Provides named contexts that bundle connection parameters (host, token, org)
//! for switching between multiple TFE/HCP instances.

mod commands;
mod models;
mod resolve;
mod store;

pub use commands::run_context_command;
pub use models::{Context, ContextConfig};
pub use resolve::resolve_active_context;
pub use store::ContextStore;
