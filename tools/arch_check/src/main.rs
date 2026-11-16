//! Layer and naming checks for the Aeris Solid workspace.
//!
//! Reads every `Cargo.toml` and fails on a dependency the AGENTS.md table
//! forbids. Also fails on a provider name outside its own crate, the
//! composition root's registration list, and provider artwork.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(serde::Deserialize)]
struct Manifest {
    package: Package,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, toml::Value>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, toml::Value>,
}

#[derive(serde::Deserialize)]
struct Package {
    name: String,
}

#[derive(serde::Deserialize)]
struct Workspace {
    workspace: WorkspaceBody,
}

#[derive(serde::Deserialize)]
struct WorkspaceBody {
    members: Vec<String>,
}

/// Workspace crates a layer may depend on, keyed by the layer's crate name.
fn allowed_dependencies(crate_name: &str, all: &BTreeSet<String>) -> Option<BTreeSet<String>> {
    let set = |names: &[&str]| -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    };
    match crate_name {
        "aeris_domain" => Some(BTreeSet::new()),
        "aeris_provider_api" => Some(set(&["aeris_domain"])),
        "aeris_provider_kit" => Some(set(&["aeris_domain", "aeris_provider_api"])),
        "aeris_market_runtime" => Some(set(&["aeris_domain", "aeris_provider_api"])),
        "aeris_trading_runtime" | "aeris_account_runtime" => Some(set(&["aeris_domain"])),
        "aeris_app_protocol" => Some(set(&["aeris_domain"])),
        "aeris_app_core" => Some(all.clone()),
        "aeris-solid" | "aeris_web_host" => Some(set(&["aeris_app_core", "aeris_app_protocol"])),
        name if name.starts_with("aeris_provider_") => Some(set(&[
            "aeris_domain",
            "aeris_provider_api",
            "aeris_provider_kit",
        ])),
        _ => None,
    }
}

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut violations: Vec<String> = Vec::new();

    let workspace_text = match fs::read_to_string(root.join("Cargo.toml")) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("arch_check: cannot read workspace manifest: {error}");
            return ExitCode::FAILURE;
        }
    };
    let workspace: Workspace = match toml::from_str(&workspace_text) {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!("arch_check: cannot parse workspace manifest: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut manifests: BTreeMap<String, PathBuf> = BTreeMap::new();
    for member in &workspace.workspace.members {
        let manifest_path = root.join(member).join("Cargo.toml");
        let text = match fs::read_to_string(&manifest_path) {
            Ok(text) => text,
            Err(error) => {
                violations.push(format!("{member}: cannot read Cargo.toml: {error}"));
                continue;
            }
        };
        let manifest: Manifest = match toml::from_str(&text) {
            Ok(manifest) => manifest,
            Err(error) => {
                violations.push(format!("{member}: cannot parse Cargo.toml: {error}"));
                continue;
            }
        };
        manifests.insert(manifest.package.name.clone(), manifest_path);
    }
    // The desktop shell is its own workspace so it keeps src-tauri/Cargo.lock.
    let tauri_manifest = root.join("src-tauri/Cargo.toml");
    if tauri_manifest.is_file()
        && let Ok(text) = fs::read_to_string(&tauri_manifest)
        && let Ok(manifest) = toml::from_str::<Manifest>(&text)
    {
        manifests.insert(manifest.package.name.clone(), tauri_manifest);
    }

    let all: BTreeSet<String> = manifests.keys().cloned().collect();
    for (name, path) in &manifests {
        // Tools are checks, not application layers; the table does not bind them.
        if path.starts_with(root.join("tools")) {
            continue;
        }
        let Some(allowed) = allowed_dependencies(name, &all) else {
            violations.push(format!("{name}: no dependency rule recorded in AGENTS.md"));
            continue;
        };
        let text = fs::read_to_string(path).unwrap_or_default();
        let Ok(manifest) = toml::from_str::<Manifest>(&text) else {
            continue;
        };
        for (dependency, _) in manifest
            .dependencies
            .iter()
            .chain(manifest.build_dependencies.iter())
            .chain(manifest.dev_dependencies.iter())
        {
            if all.contains(dependency) && !allowed.contains(dependency) {
                violations.push(format!(
                    "{name}: dependency on {dependency} is forbidden by the layer table"
                ));
            }
        }
    }

    let providers_dir = root.join("crates/providers");
    let mut provider_names: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&providers_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                provider_names.push(name.to_string());
            }
        }
    }
    if provider_names.is_empty() {
        violations.push("crates/providers: no provider crate found".to_string());
    }

    let mut sources: Vec<PathBuf> = Vec::new();
    collect_sources(&root.join("crates"), &mut sources);
    collect_sources(&root.join("src-tauri/src"), &mut sources);
    collect_sources(&root.join("src"), &mut sources);
    for path in sources {
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let text = fs::read_to_string(&path).unwrap_or_default();
        let owned = owned_provider(relative, &provider_names);
        let registration = relative == Path::new("crates/app_core/src/providers.rs");
        for (number, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            for provider in &provider_names {
                if !lower.contains(provider.as_str()) {
                    continue;
                }
                if owned.as_deref() == Some(provider.as_str()) || registration {
                    continue;
                }
                violations.push(format!(
                    "{}:{}: provider name {provider} outside its own crate",
                    relative.display(),
                    number + 1
                ));
            }
        }
    }

    if violations.is_empty() {
        println!(
            "arch_check: {} crates, {} providers, layer and naming rules pass",
            manifests.len(),
            provider_names.len()
        );
        return ExitCode::SUCCESS;
    }
    for violation in &violations {
        eprintln!("arch_check: {violation}");
    }
    eprintln!("arch_check: {} violation(s)", violations.len());
    ExitCode::FAILURE
}

/// The provider crate a source file belongs to, if any.
fn owned_provider(relative: &Path, providers: &[String]) -> Option<String> {
    let mut components = relative.components();
    if components.next()?.as_os_str() != "crates" {
        return None;
    }
    if components.next()?.as_os_str() != "providers" {
        return None;
    }
    let name = components.next()?.as_os_str().to_str()?;
    providers
        .iter()
        .find(|provider| provider.as_str() == name)
        .cloned()
}

fn collect_sources(directory: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if name == "target" || name == "node_modules" || name == "assets" {
                continue;
            }
            collect_sources(&path, out);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "ts" | "tsx")
        ) {
            out.push(path);
        }
    }
}
