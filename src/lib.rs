//! Chrome extension (Manifest V3) shell for the Orbital compiler.
//!
//! Like the Android/iOS shells, it bundles the resolved program instead of generating code: the
//! extension's service worker runs it on the JS runtime (`@almadar/runtime`) with the browser
//! integrations, and the side panel renders it with `@almadar/ui` over a message channel.

pub mod backend;
pub mod program;
pub mod shell;
pub mod shell_program;

pub use backend::ChromeBackend;
pub use program::{check_program, ChromeProgram, ProgramError};
pub use shell::ChromeShell;
