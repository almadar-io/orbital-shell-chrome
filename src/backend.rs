//! The chrome-extension backend: bundles the resolved program for the worker's runtime host and
//! writes the Manifest V3 manifest from declared facts — the app's identity, the sites its `page`
//! calls read, and the permissions its services need.

use orbital_core::generate::{Backend, GeneratedFile, GeneratedOutput};
use orbital_core::ir::OirModule;
use orbital_core::schema::{OrbitalSchema, ThemeDefinition};
use orbital_shell_typescript::{generate_theme_css, theme_font_imports};
use serde_json::{json, Value};

use crate::program::ChromeProgram;

pub struct ChromeBackend {
    resolved_schema: Option<OrbitalSchema>,
    program: ChromeProgram,
}

impl ChromeBackend {
    pub fn new(resolved_schema: OrbitalSchema, program: ChromeProgram) -> Self {
        Self {
            resolved_schema: Some(resolved_schema),
            program,
        }
    }
}

/// Chrome extension versions are one to four dot-separated integers; anything after a `-`/`+` is dropped.
fn extension_version(version: &str) -> String {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core
        .split('.')
        .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if parts.is_empty() {
        "0.0.0".to_string()
    } else {
        parts.into_iter().take(4).collect::<Vec<_>>().join(".")
    }
}

/// The permissions the program's services need, plus the two the host itself uses.
fn permissions(program: &ChromeProgram) -> Vec<&'static str> {
    let mut out = vec!["sidePanel", "storage"];
    if program.services.iter().any(|s| s == "page") {
        out.push("tabs");
    }
    if program.services.iter().any(|s| s == "knowledge") {
        out.push("identity");
    }
    out
}

/// The program's own themes as stylesheets, plus `themes.css` importing them, which the panel's
/// stylesheet imports. Built-in themes ship with `@almadar/ui`.
pub fn theme_files(themes: &[ThemeDefinition]) -> Vec<GeneratedFile> {
    let mut index = String::new();
    for url in theme_font_imports(themes) {
        index.push_str(&format!("@import url(\"{url}\");\n"));
    }
    let mut files = Vec::new();
    for theme in themes {
        index.push_str(&format!("@import './themes/{}.css';\n", theme.name));
        files.push(GeneratedFile::new(
            format!("themes/{}.css", theme.name),
            generate_theme_css(theme),
        ));
    }
    files.push(GeneratedFile::new("themes.css", index));
    files
}

const MODEL_HOSTS: [&str; 2] = ["https://huggingface.co/*", "https://*.hf.co/*"];
const EMBED_CSP: &str = "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'";

const EMBEDDER_REAL: &str = r#"import { env, pipeline } from '@huggingface/transformers';
import { transformersEmbedder, type OnDeviceEmbedderPort } from '@almadar/integrations/browser';

env.allowLocalModels = false;
env.backends.onnx.wasm.numThreads = 1;

export const embedder: OnDeviceEmbedderPort | undefined = transformersEmbedder(
  () => pipeline('feature-extraction', 'Xenova/multilingual-e5-small', { dtype: 'q8', device: 'wasm' }),
  { prefix: 'query: ' },
);
"#;

const EMBEDDER_STUB: &str = "import type { OnDeviceEmbedderPort } from '@almadar/integrations/browser';\n\nexport const embedder: OnDeviceEmbedderPort | undefined = undefined;\n";

/// The worker's embedder wiring and the files `finalize.mjs` copies into `dist/`, both declared by
/// whether the program calls `llm` `embed`.
pub fn embedder_files(program: &ChromeProgram) -> Vec<GeneratedFile> {
    let (embedder, assets) = if program.embeds_on_device {
        (
            EMBEDDER_REAL,
            json!([{
                "from": "node_modules/onnxruntime-web/dist/ort-wasm-simd-threaded.jsep.wasm",
                "to": "ort-wasm-simd-threaded.jsep.wasm"
            }]),
        )
    } else {
        (EMBEDDER_STUB, json!([]))
    };
    vec![
        GeneratedFile::new("embedder.ts", embedder.to_string()),
        GeneratedFile::new(
            "assets.json",
            serde_json::to_string_pretty(&assets).unwrap_or_default(),
        ),
    ]
}

pub fn manifest(
    name: &str,
    version: &str,
    description: Option<&str>,
    program: &ChromeProgram,
) -> Value {
    let mut m = json!({
        "manifest_version": 3,
        "name": name,
        "version": extension_version(version),
        "background": { "service_worker": "worker.js", "type": "module" },
        "action": { "default_title": name },
        "side_panel": { "default_path": "panel.html" },
        "permissions": permissions(program),
    });
    if let Some(d) = description {
        m["description"] = json!(d);
    }
    let mut host_permissions = program.host_patterns.clone();
    if program.embeds_on_device {
        host_permissions.extend(MODEL_HOSTS.iter().map(|h| h.to_string()));
        m["content_security_policy"] = json!({ "extension_pages": EMBED_CSP });
    }
    if !host_permissions.is_empty() {
        m["host_permissions"] = json!(host_permissions);
    }
    if !program.host_patterns.is_empty() {
        m["content_scripts"] = json!([{ "matches": program.host_patterns, "js": ["content.js"], "run_at": "document_idle" }]);
    }
    m
}

impl Backend for ChromeBackend {
    fn generate(&self, module: &OirModule) -> GeneratedOutput {
        let mut output = GeneratedOutput::default();
        match &self.resolved_schema {
            Some(schema) => match orbital_core::util::canonical_json::to_canonical_string_pretty(schema) {
                Ok(json) => output.files.push(GeneratedFile::new("program.json", json)),
                Err(e) => output
                    .warnings
                    .push(format!("could not serialize the resolved program: {e}")),
            },
            None => output
                .warnings
                .push("no resolved program to bundle".to_string()),
        }
        output.files.extend(theme_files(&module.themes));
        let manifest = manifest(
            &module.name,
            &module.version,
            module.description.as_deref(),
            &self.program,
        );
        output.files.extend(embedder_files(&self.program));
        output.files.push(GeneratedFile::new(
            "manifest.json",
            serde_json::to_string_pretty(&manifest).unwrap_or_default(),
        ));
        output
    }

    fn name(&self) -> &'static str {
        "chrome-extension"
    }

    fn supports_browser_stored_entities(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(patterns: &[&str], services: &[&str]) -> ChromeProgram {
        ChromeProgram {
            host_patterns: patterns.iter().map(|s| s.to_string()).collect(),
            services: services.iter().map(|s| s.to_string()).collect(),
            embeds_on_device: false,
        }
    }

    fn embedding_program(patterns: &[&str]) -> ChromeProgram {
        ChromeProgram {
            embeds_on_device: true,
            ..program(patterns, &["llm"])
        }
    }

    #[test]
    fn manifest_carries_the_program_sites_and_the_permissions_its_services_need() {
        let m = manifest(
            "Feed",
            "1.2.3",
            Some("d"),
            &program(&["https://example.com/*"], &["knowledge", "llm", "page"]),
        );
        assert_eq!(m["manifest_version"], 3);
        assert_eq!(m["host_permissions"], json!(["https://example.com/*"]));
        assert_eq!(
            m["content_scripts"][0]["matches"],
            json!(["https://example.com/*"])
        );
        assert_eq!(
            m["permissions"],
            json!(["sidePanel", "storage", "tabs", "identity"])
        );
        assert_eq!(m["background"]["service_worker"], "worker.js");
    }

    #[test]
    fn control_a_program_that_reads_no_site_gets_no_host_access_and_no_content_script() {
        let m = manifest("App", "1.0.0", None, &program(&[], &["llm"]));
        assert!(m.get("host_permissions").is_none());
        assert!(m.get("content_scripts").is_none());
        assert_eq!(m["permissions"], json!(["sidePanel", "storage"]));
    }

    #[test]
    fn a_program_calling_llm_embed_gets_the_csp_model_hosts_embedder_and_wasm_asset() {
        let m = manifest(
            "App",
            "1.0.0",
            None,
            &embedding_program(&["https://example.com/*"]),
        );
        assert_eq!(
            m["content_security_policy"]["extension_pages"],
            "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'"
        );
        assert_eq!(
            m["host_permissions"],
            json!([
                "https://example.com/*",
                "https://huggingface.co/*",
                "https://*.hf.co/*"
            ])
        );
        assert_eq!(
            m["content_scripts"][0]["matches"],
            json!(["https://example.com/*"])
        );
        let files = embedder_files(&embedding_program(&[]));
        assert_eq!(files[0].path, "embedder.ts");
        assert!(files[0].content.contains("@huggingface/transformers"));
        assert!(files[0].content.contains("transformersEmbedder"));
        let assets: Value = serde_json::from_str(&files[1].content).expect("assets json");
        assert_eq!(files[1].path, "assets.json");
        assert_eq!(
            assets,
            json!([{ "from": "node_modules/onnxruntime-web/dist/ort-wasm-simd-threaded.jsep.wasm", "to": "ort-wasm-simd-threaded.jsep.wasm" }])
        );
    }

    #[test]
    fn control_a_program_without_llm_embed_gets_no_csp_no_model_hosts_and_the_stub() {
        let p = program(&["https://example.com/*"], &["llm"]);
        let m = manifest("App", "1.0.0", None, &p);
        assert!(m.get("content_security_policy").is_none());
        assert_eq!(m["host_permissions"], json!(["https://example.com/*"]));
        let files = embedder_files(&p);
        assert!(!files[0].content.contains("@huggingface"));
        assert!(files[0].content.contains("= undefined;"));
        assert_eq!(files[1].content.trim(), "[]");
    }

    #[test]
    fn edge_embed_without_host_patterns_holds_only_the_model_hosts_and_no_content_script() {
        let m = manifest("App", "1.0.0", None, &embedding_program(&[]));
        assert_eq!(
            m["host_permissions"],
            json!(["https://huggingface.co/*", "https://*.hf.co/*"])
        );
        assert!(m.get("content_scripts").is_none());
    }

    #[test]
    fn the_generated_output_carries_the_embedder_and_assets_files() {
        let schema: OrbitalSchema =
            serde_json::from_value(json!({ "name": "app", "orbitals": [] })).expect("schema");
        let module: OirModule = serde_json::from_value(json!({
            "name": "app", "version": "1.0.0", "entities": [], "traits": [], "pages": [], "metadata": {}
        }))
        .expect("module");
        let out = ChromeBackend::new(schema, embedding_program(&[])).generate(&module);
        let paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"embedder.ts") && paths.contains(&"assets.json"));
    }

    fn theme(name: &str) -> ThemeDefinition {
        serde_json::from_value(json!({ "name": name, "tokens": {} })).expect("theme fixture")
    }

    #[test]
    fn a_programs_own_themes_become_stylesheets_the_panel_imports() {
        let files = theme_files(&[theme("sunset")]);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["themes/sunset.css", "themes.css"]);
        assert!(files[1].content.contains("@import './themes/sunset.css';"));
    }

    #[test]
    fn control_a_program_without_themes_still_gets_the_index_the_panel_imports() {
        let files = theme_files(&[]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "themes.css");
        assert!(!files[0].content.contains("@import"));
    }

    #[test]
    fn edge_versions_become_chrome_versions() {
        assert_eq!(extension_version("2.0.0-beta.1"), "2.0.0");
        assert_eq!(extension_version("1.2.3.4.5"), "1.2.3.4");
        assert_eq!(extension_version("dev"), "0.0.0");
    }
}
