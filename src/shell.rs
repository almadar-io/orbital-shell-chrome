//! Chrome extension shell: the Manifest V3 scaffold whose service worker hosts the bundled program.

use orbital_core::shell::{Shell, ShellError};
use std::path::{Path, PathBuf};

/// Chrome extension (Manifest V3) shell
pub struct ChromeShell {
    template_path: PathBuf,
}

impl ChromeShell {
    /// Create a new chrome-extension shell
    pub fn new() -> Self {
        let template_path = Self::find_template_path();
        Self { template_path }
    }

    /// Find template path at runtime using multiple strategies.
    ///
    /// Mirrors the native-shell lookup (android/ios/rust/mobile): env var
    /// (existence-checked), exe-relative, repo-root
    /// (this crate's `template/`), compile-time fallback.
    /// `chrome-shell` is a self-contained committed scaffold under
    /// `templates/`, like every other shell.
    fn find_template_path() -> PathBuf {
        // The one template lookup (`ORBITAL_SHELLS_DIR` → beside the binary → the checkout).
        orbital_core::renderer_packs::shell_template("chrome-shell", std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    /// Create with explicit template path
    pub fn with_template_path(template_path: impl Into<PathBuf>) -> Self {
        Self {
            template_path: template_path.into(),
        }
    }
}

impl Default for ChromeShell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell for ChromeShell {
    fn name(&self) -> &'static str {
        "chrome-extension"
    }

    fn description(&self) -> &'static str {
        "Chrome extension (Manifest V3) hosting the program in its service worker"
    }

    fn extensions(&self) -> &[&'static str] {
        &["ts", "tsx"]
    }

    fn template_path(&self) -> &Path {
        &self.template_path
    }

    fn generated_dir(&self) -> &str {
        "src/generated"
    }

    fn build_command(&self) -> &str {
        "pnpm run build"
    }

    fn dev_command(&self) -> &str {
        "pnpm run dev"
    }

    fn test_command(&self) -> &str {
        "pnpm run test"
    }

    fn init_project(&self, target_dir: &Path, project_name: &str) -> Result<(), ShellError> {
        self.copy_template(target_dir)?;

        let package_json_path = target_dir.join("package.json");
        if package_json_path.exists() {
            let content = std::fs::read_to_string(&package_json_path)?;
            let updated = content.replace(
                "\"name\": \"orbital-app\"",
                &format!("\"name\": \"{}\"", project_name),
            );
            std::fs::write(&package_json_path, updated)?;
        }

        Ok(())
    }

    fn copy_template(&self, output_dir: &Path) -> Result<(), ShellError> {
        if !self.template_path.exists() {
            return Err(ShellError::NotFound(
                self.template_path.display().to_string(),
            ));
        }

        copy_dir_recursive(&self.template_path, output_dir)?;
        Ok(())
    }

    fn validate_structure(&self, output_dir: &Path) -> Result<(), ShellError> {
        let required_files = ["package.json"];

        for file in &required_files {
            let path = output_dir.join(file);
            if !path.exists() {
                return Err(ShellError::InvalidStructure {
                    path: path.clone(),
                    message: format!("Missing required file: {}", file),
                });
            }
        }

        Ok(())
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), ShellError> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        if file_name_str == "generated"
            || file_name_str == "node_modules"
            || file_name_str == ".git"
        {
            continue;
        }

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_properties() {
        let shell = ChromeShell::new();
        assert_eq!(shell.name(), "chrome-extension");
        assert_eq!(shell.extensions(), &["ts", "tsx"]);
        assert_eq!(shell.generated_dir(), "src/generated");
        assert_eq!(shell.build_command(), "pnpm run build");
        assert_eq!(shell.dev_command(), "pnpm run dev");
        assert_eq!(shell.test_command(), "pnpm run test");
    }
}
