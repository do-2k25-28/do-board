use serde::Deserialize;

#[derive(Deserialize)]
struct WorkspaceCargo {
    workspace: Workspace,
}

#[derive(Deserialize)]
struct Workspace {
    metadata: Metadata,
}

#[derive(Deserialize)]
struct Metadata {
    app_name: String,
}

fn main() {
    println!("cargo:rerun-if-changed=../Cargo.toml");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_toml = std::path::PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("Cargo.toml");

    let content = std::fs::read_to_string(workspace_toml).unwrap();
    let cargo: WorkspaceCargo = toml::from_str(&content).unwrap();

    println!(
        "cargo:rustc-env=APP_NAME={}",
        cargo.workspace.metadata.app_name
    );
}
