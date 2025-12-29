//! TFE API client module
//!
//! This module provides functionality to interact with Terraform Enterprise API.

mod client;
mod credentials;
mod models;

pub use client::TfeClient;
pub use credentials::TokenResolver;
pub use models::{Organization, Workspace, WorkspaceAttributes};
