//! UI utilities for terminal output
//!
//! This module provides user interface components like progress spinners
//! and confirmation prompts.

mod confirm;
mod spinner;

pub use confirm::{confirm_action, confirm_large_pagination, LargePaginationInfo};
pub use spinner::{
    create_spinner, finish_spinner, finish_spinner_with_message, finish_spinner_with_status,
};
