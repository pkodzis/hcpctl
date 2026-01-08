//! UI utilities for terminal output
//!
//! This module provides user interface components like progress spinners.

mod spinner;

pub use spinner::{
    create_spinner, finish_spinner, finish_spinner_with_message, finish_spinner_with_status,
};
