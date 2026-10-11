//! What the chrome-extension shell requires of a program, checked on the resolved program before any
//! file is written. The extension's service worker is the program's only host, so every service it
//! calls must declare the browser runtime and every persistent entity must live in the browser. The
//! sites the program reads come from its `page` calls' `match` values, never from a guess.

use std::collections::{BTreeSet, HashMap};

use orbital_core::effects::runs_on::{declares_runtime, SERVICE_RUNTIME_UNSUPPORTED};
use serde_json::Value;

/// What the shell derives from an accepted program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeProgram {
    /// URL match patterns the program's `page` calls read: the extension's host permissions and
    /// content-script matches.
    pub host_patterns: Vec<String>,
    /// Services the program calls, each one declaring the browser runtime.
    pub services: Vec<String>,
    /// The program calls `llm` action `embed`, so the worker needs an on-device embedder.
    pub embeds_on_device: bool,
}

/// One reason a program cannot compile to the chrome-extension shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// The page actions whose `match` names the sites the program reads.
const PAGE_MATCH_ACTIONS: [&str; 3] = ["watch", "listTabs", "openTab"];

/// Check a resolved program (its `.orb` JSON) against the services' declared runtimes.
pub fn check_program(
    program: &Value,
    service_runtimes: &HashMap<String, Vec<String>>,
) -> Result<ChromeProgram, Vec<ProgramError>> {
    let mut errors = Vec::new();
    let mut patterns = BTreeSet::new();
    let mut services = BTreeSet::new();
    let mut embeds_on_device = false;

    for orbital in array(program, "orbitals") {
        let orbital_name = string(orbital, "name");
        if let Some(entity) = orbital.get("entity") {
            if string(entity, "persistence") == "persistent"
                && entity.get("local") != Some(&Value::Bool(true))
            {
                errors.push(ProgramError {
                    code: "ORB_SHELL_SERVER_ENTITY_UNSUPPORTED",
                    message: format!(
                        "{}.{} is a server-stored entity; a chrome extension has no server. Declare it `[persistent: …, local]` or `[runtime]`.",
                        orbital_name,
                        string(entity, "name")
                    ),
                });
            }
        }
        for t in array(orbital, "traits") {
            let trait_name = string(t, "name");
            let config = t.get("config");
            let mut calls = Vec::new();
            collect_call_services(t, &mut calls);
            for call in calls {
                let service = call.get(1).and_then(Value::as_str).unwrap_or_default();
                let method = call.get(2).and_then(Value::as_str).unwrap_or_default();
                let browser = service_runtimes
                    .get(service)
                    .is_some_and(|r| declares_runtime(r, "browser"));
                if !browser {
                    errors.push(ProgramError {
                        code: SERVICE_RUNTIME_UNSUPPORTED,
                        message: format!(
                            "{}.{} calls service '{}', which declares no browser runtime; a chrome extension can only call services that run in the browser.",
                            orbital_name, trait_name, service
                        ),
                    });
                    continue;
                }
                services.insert(service.to_string());
                embeds_on_device |= service == "llm" && method == "embed";
                if service == "page" && PAGE_MATCH_ACTIONS.contains(&method) {
                    match call.get(3).and_then(|args| args.get("match")).map(|m| resolve(m, config)) {
                        Some(Some(pattern)) => {
                            patterns.insert(pattern);
                        }
                        Some(None) => errors.push(ProgramError {
                            code: "ORB_SHELL_PAGE_MATCH_UNRESOLVED",
                            message: format!(
                                "{}.{} calls page '{}' with a `match` that is not a literal URL pattern or a config knob holding one; the extension's site access must be known at build time.",
                                orbital_name, trait_name, method
                            ),
                        }),
                        None => {}
                    }
                }
            }
        }
    }

    let has_root =
        array(program, "orbitals").any(|o| array(o, "pages").any(|p| string(p, "path") == "/"));
    if !has_root {
        errors.push(ProgramError {
            code: "ORB_SHELL_ROOT_PAGE_MISSING",
            message: "A chrome extension shows and keeps running the program's `/` page (the side panel's page, mounted by the worker at start); declare a page at `/`.".to_string(),
        });
    }

    if errors.is_empty() {
        Ok(ChromeProgram {
            host_patterns: patterns.into_iter().collect(),
            services: services.into_iter().collect(),
            embeds_on_device,
        })
    } else {
        Err(errors)
    }
}

/// A `match` value: a literal URL match pattern, or `@config.<knob>` whose default is one.
fn resolve(value: &Value, config: Option<&Value>) -> Option<String> {
    let s = value.as_str()?;
    let pattern = match s.strip_prefix("@config.") {
        None => s,
        Some(knob) => config?.get(knob)?.get("default")?.as_str()?,
    };
    is_match_pattern(pattern).then(|| pattern.to_string())
}

/// Chrome's match-pattern grammar: `<all_urls>`, or `<scheme>://<host><path>` where scheme is
/// `*|http|https|file|ftp`, host is `*`, `*.<domain>` or a domain, and path starts with `/`.
fn is_match_pattern(p: &str) -> bool {
    if p == "<all_urls>" {
        return true;
    }
    let Some((scheme, rest)) = p.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "*" | "http" | "https" | "file" | "ftp") {
        return false;
    }
    let Some(slash) = rest.find('/') else {
        return false;
    };
    let host = &rest[..slash];
    let host_ok = host == "*"
        || (host.starts_with("*.") && host.len() > 2 && !host[2..].contains('*'))
        || (!host.is_empty() && !host.contains('*'));
    host_ok || (scheme == "file" && host.is_empty())
}

/// Every `["call-service", service, method, args, …]` form anywhere under `value`.
fn collect_call_services<'a>(value: &'a Value, out: &mut Vec<&'a Vec<Value>>) {
    match value {
        Value::Array(items) => {
            if items.first().and_then(Value::as_str) == Some("call-service") {
                out.push(items);
            }
            for item in items {
                collect_call_services(item, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_call_services(v, out);
            }
        }
        _ => {}
    }
}

fn array<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn runtimes() -> HashMap<String, Vec<String>> {
        HashMap::from([
            ("page".to_string(), vec!["browser".to_string()]),
            (
                "llm".to_string(),
                vec!["node".to_string(), "browser".to_string()],
            ),
            ("stripe".to_string(), vec!["node".to_string()]),
        ])
    }

    fn program(entity: Value, traits: Value) -> Value {
        json!({ "name": "app", "orbitals": [{ "name": "Feed", "entity": entity, "traits": traits, "pages": [{ "name": "Home", "path": "/" }] }] })
    }

    fn local_entity() -> Value {
        json!({ "name": "Item", "persistence": "persistent", "local": true })
    }

    fn trait_with(effects: Value, config: Value) -> Value {
        json!([{ "name": "Watcher", "config": config, "stateMachine": { "transitions": [{ "event": "INIT", "effects": effects }] } }])
    }

    #[test]
    fn accepts_a_browser_local_program_and_collects_literal_and_config_match_patterns() {
        let p = program(
            local_entity(),
            trait_with(
                json!([
                    ["call-service", "page", "watch", { "match": "https://example.com/*", "selector": ".i" }],
                    ["if", true, ["call-service", "page", "listTabs", { "match": "@config.site" }]],
                    ["call-service", "llm", "classify", { "text": "t" }]
                ]),
                json!({ "site": { "default": "https://*.example.org/*" } }),
            ),
        );
        let out = check_program(&p, &runtimes()).expect("accepted");
        assert_eq!(
            out.host_patterns,
            vec![
                "https://*.example.org/*".to_string(),
                "https://example.com/*".to_string()
            ]
        );
        assert_eq!(out.services, vec!["llm".to_string(), "page".to_string()]);
    }

    #[test]
    fn a_program_calling_llm_embed_needs_an_on_device_embedder() {
        let p = program(
            local_entity(),
            trait_with(
                json!([["if", true, ["call-service", "llm", "embed", { "texts": [] }]]]),
                json!({}),
            ),
        );
        assert!(
            check_program(&p, &runtimes())
                .expect("accepted")
                .embeds_on_device
        );
    }

    #[test]
    fn control_other_llm_actions_and_other_services_do_not_need_an_embedder() {
        let p = program(
            local_entity(),
            trait_with(
                json!([
                    ["call-service", "llm", "classifyMany", {}],
                    ["call-service", "page", "embed", {}]
                ]),
                json!({}),
            ),
        );
        assert!(
            !check_program(&p, &runtimes())
                .expect("accepted")
                .embeds_on_device
        );
    }

    #[test]
    fn refuses_a_service_without_a_browser_runtime() {
        let p = program(
            local_entity(),
            trait_with(
                json!([["call-service", "stripe", "createPaymentIntent", {}]]),
                json!({}),
            ),
        );
        let errs = check_program(&p, &runtimes()).unwrap_err();
        assert_eq!(
            errs.iter().map(|e| e.code).collect::<Vec<_>>(),
            vec!["ORB_SHELL_SERVICE_RUNTIME_UNSUPPORTED"]
        );
    }

    #[test]
    fn edge_an_unregistered_service_is_refused_too() {
        let p = program(
            local_entity(),
            trait_with(json!([["call-service", "nope", "x", {}]]), json!({})),
        );
        assert_eq!(
            check_program(&p, &runtimes()).unwrap_err()[0].code,
            "ORB_SHELL_SERVICE_RUNTIME_UNSUPPORTED"
        );
    }

    #[test]
    fn refuses_a_program_without_a_root_page() {
        let p = json!({ "name": "app", "orbitals": [{ "name": "Feed", "entity": local_entity(), "traits": [], "pages": [{ "name": "P", "path": "/panel" }] }] });
        assert_eq!(
            check_program(&p, &runtimes()).unwrap_err()[0].code,
            "ORB_SHELL_ROOT_PAGE_MISSING"
        );
    }

    #[test]
    fn refuses_a_server_stored_entity() {
        let p = program(
            json!({ "name": "Item", "persistence": "persistent" }),
            json!([]),
        );
        assert_eq!(
            check_program(&p, &runtimes()).unwrap_err()[0].code,
            "ORB_SHELL_SERVER_ENTITY_UNSUPPORTED"
        );
    }

    #[test]
    fn control_runtime_entities_are_browser_resident() {
        let p = program(
            json!({ "name": "Item", "persistence": "runtime" }),
            json!([]),
        );
        assert!(check_program(&p, &runtimes()).is_ok());
    }

    #[test]
    fn refuses_a_page_match_it_cannot_resolve_at_build_time() {
        let p = program(
            local_entity(),
            trait_with(
                json!([["call-service", "page", "watch", { "match": "@entity.site" }]]),
                json!({}),
            ),
        );
        assert_eq!(
            check_program(&p, &runtimes()).unwrap_err()[0].code,
            "ORB_SHELL_PAGE_MATCH_UNRESOLVED"
        );
    }

    #[test]
    fn refuses_a_literal_that_is_not_a_url_match_pattern() {
        for bad in [
            "example.com",
            "https://ex*ample.com/",
            "javascript://x/",
            "https://example.com",
        ] {
            let p = program(
                local_entity(),
                trait_with(
                    json!([["call-service", "page", "watch", { "match": bad }]]),
                    json!({}),
                ),
            );
            assert_eq!(
                check_program(&p, &runtimes()).unwrap_err()[0].code,
                "ORB_SHELL_PAGE_MATCH_UNRESOLVED",
                "{bad}"
            );
        }
    }

    #[test]
    fn control_valid_patterns_are_accepted() {
        for good in [
            "<all_urls>",
            "*://example.com/*",
            "https://*.example.com/watch*",
            "file:///docs/*",
        ] {
            assert!(is_match_pattern(good), "{good}");
        }
    }

    #[test]
    fn an_open_tab_match_grants_access_to_the_pages_it_will_read() {
        let p = program(
            local_entity(),
            trait_with(
                json!([["call-service", "page", "openTab", { "url": "@config.searchUrl", "match": "@config.resultsMatch", "active": false }]]),
                json!({ "resultsMatch": { "default": "https://search.example.com/*" } }),
            ),
        );
        assert_eq!(
            check_program(&p, &runtimes()).unwrap().host_patterns,
            vec!["https://search.example.com/*".to_string()]
        );
    }

    #[test]
    fn control_an_open_tab_without_a_match_grants_nothing_and_is_not_an_error() {
        let p = program(
            local_entity(),
            trait_with(
                json!([["call-service", "page", "openTab", { "url": "@payload.url", "active": true }]]),
                json!({}),
            ),
        );
        assert_eq!(
            check_program(&p, &runtimes()).unwrap().host_patterns,
            Vec::<String>::new()
        );
    }

    #[test]
    fn control_page_actions_without_a_match_add_no_pattern() {
        let p = program(
            local_entity(),
            trait_with(
                json!([["call-service", "page", "apply", { "tab": 1 }]]),
                json!({}),
            ),
        );
        assert_eq!(
            check_program(&p, &runtimes()).unwrap().host_patterns,
            Vec::<String>::new()
        );
    }
}
