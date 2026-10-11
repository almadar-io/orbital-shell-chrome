//! `orb-shell-chrome`: the Chrome extension shell as a program (`orbital_core::shell_protocol`).

use orbital_core::project::write_project;
use orbital_core::shell_protocol::{
    description, written_files, GenerateRequest, GenerateResponse, ShellDescription, ShellFailure, ShellProgram,
    CODE_BAD_REQUEST, CODE_GENERATE_FAILED,
};
use std::path::Path;

pub struct ChromeProgram;

fn resolved(request: &GenerateRequest) -> Result<orbital_core::OrbitalSchema, ShellFailure> {
    request
        .resolved
        .clone()
        .ok_or_else(|| ShellFailure::new(CODE_BAD_REQUEST, "the chrome shell needs the resolved program"))
}

impl ShellProgram for ChromeProgram {
    fn describe(&self) -> ShellDescription {
        description("chrome", env!("CARGO_PKG_VERSION"), true, Vec::new())
    }

    fn generate(&self, request: &GenerateRequest, output: &Path) -> Result<GenerateResponse, ShellFailure> {
        let resolved = resolved(request)?;
        let program_json = serde_json::to_value(&resolved).map_err(|e| ShellFailure::new(CODE_GENERATE_FAILED, e.to_string()))?;
        let runtimes = match &request.write.service_runtimes {
            Some(runtimes) => runtimes.clone(),
            None => orbital_core::language::registry::baked_service_registry().runtimes.clone(),
        };
        let program = crate::check_program(&program_json, &runtimes).map_err(|errors| {
            ShellFailure::new(CODE_GENERATE_FAILED, errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))
        })?;
        let backend = crate::ChromeBackend::new(resolved, program);
        let generated = write_project(&request.module, output, &crate::ChromeShell::new(), &backend, &request.write)?;
        Ok(GenerateResponse {
            files: written_files(&generated),
            warnings: generated.warnings,
            next_steps: vec![
                format!("cd {}", output.display()),
                "pnpm install && pnpm build".to_string(),
                "Load dist/ as an unpacked extension at chrome://extensions".to_string(),
            ],
        })
    }
}
