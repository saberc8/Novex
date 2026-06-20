use std::fs;
use std::path::Path;

const BACKEND_PORT: &str = "62601";
const ADMIN_PORT: &str = "62602";
const TRAINING_WEB_PORT: &str = "62603";
const NOTEBOOKLM_PORT: &str = "62604";
const AGENT_WORKSPACE_PORT: &str = "62605";
const CODEX_APP_POC_PORT: &str = "62606";

#[test]
fn poc_env_examples_define_the_626xx_port_contract() {
    let root = repo_root();
    let root_env = read(root.join(".env.example"));
    let backend_env = read(root.join("backend/.env.example"));

    assert!(root_env.contains(&format!("HTTP_PORT={BACKEND_PORT}")));
    assert!(root_env.contains(&format!("ADMIN_PORT={ADMIN_PORT}")));
    assert!(root_env.contains(&format!("TRAINING_WEB_PORT={TRAINING_WEB_PORT}")));
    assert!(root_env.contains(&format!("NOTEBOOKLM_PORT={NOTEBOOKLM_PORT}")));
    assert!(root_env.contains(&format!("AGENT_WORKSPACE_PORT={AGENT_WORKSPACE_PORT}")));
    assert!(root_env.contains(&format!("CODEX_APP_POC_PORT={CODEX_APP_POC_PORT}")));
    assert!(root_env.contains(&format!(
        "NEXT_PUBLIC_API_BASE_URL=http://localhost:{BACKEND_PORT}"
    )));
    assert!(root_env.contains(&format!(
        "PARSER_BACKEND_BASE_URL=http://127.0.0.1:{BACKEND_PORT}"
    )));

    assert!(backend_env.contains(&format!("HTTP_PORT={BACKEND_PORT}")));
    assert!(backend_env.contains(&format!("http://localhost:{CODEX_APP_POC_PORT}")));
    assert!(backend_env.contains(&format!("http://127.0.0.1:{CODEX_APP_POC_PORT}")));
}

#[test]
fn frontend_dev_scripts_use_the_same_626xx_port_contract() {
    let root = repo_root();

    assert_package_dev_script(root.join("admin/package.json"), "ADMIN_PORT", ADMIN_PORT);
    assert_package_dev_script(
        root.join("apps/training-web/package.json"),
        "TRAINING_WEB_PORT",
        TRAINING_WEB_PORT,
    );
    assert_package_dev_script(
        root.join("apps/notebooklm/package.json"),
        "NOTEBOOKLM_PORT",
        NOTEBOOKLM_PORT,
    );
    assert_package_dev_script(
        root.join("apps/agent-workspace/package.json"),
        "AGENT_WORKSPACE_PORT",
        AGENT_WORKSPACE_PORT,
    );
    assert_package_dev_script(
        root.join("apps/codex-app-poc/package.json"),
        "CODEX_APP_POC_PORT",
        CODEX_APP_POC_PORT,
    );
}

#[test]
fn run_poc_prints_commands_from_the_626xx_port_contract() {
    let script = read(repo_root().join("scripts/run-poc.sh"));

    assert!(script.contains(&format!("HTTP_PORT:-{BACKEND_PORT}")));
    assert!(script.contains(&format!("ADMIN_PORT:-{ADMIN_PORT}")));
    assert!(script.contains(&format!("TRAINING_WEB_PORT:-{TRAINING_WEB_PORT}")));
    assert!(script.contains(&format!("NOTEBOOKLM_PORT:-{NOTEBOOKLM_PORT}")));
    assert!(script.contains(&format!("AGENT_WORKSPACE_PORT:-{AGENT_WORKSPACE_PORT}")));
    assert!(script.contains(&format!("CODEX_APP_POC_PORT:-{CODEX_APP_POC_PORT}")));
}

fn assert_package_dev_script(path: impl AsRef<Path>, env_var: &str, port: &str) {
    let package = read(path);
    assert!(
        package.contains(&format!(
            "\"dev\": \"next dev --webpack -p ${{{env_var}:-{port}}}\""
        )),
        "{env_var} dev script should default to {port} through the webpack dev server"
    );
}

#[test]
fn backend_reqwest_keeps_system_proxy_enabled_for_local_web_search() {
    let cargo_toml = read(repo_root().join("backend/Cargo.toml"));

    assert!(
        cargo_toml.contains(r#"features = ["json", "rustls-tls", "system-proxy"]"#),
        "backend web.search must honor macOS/system proxy settings for local POC networking"
    );
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend is nested under repo root")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref()).unwrap_or_else(|error| {
        panic!("read {}: {error}", path.as_ref().display());
    })
}
