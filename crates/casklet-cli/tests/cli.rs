use std::{fs, process::Command};

#[test]
fn create_list_get_across_processes() {
    let root = std::env::temp_dir().join(format!("casklet-cli-{}", std::process::id()));
    let binary = env!("CARGO_BIN_EXE_casklet");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .env("CASKLET_DEV_ROOT", &root)
            .output()
            .unwrap()
    };
    let created = run(&["cask", "create", "My Work"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert_eq!(
        String::from_utf8(created.stdout).unwrap(),
        "Created Cask: My Work\n"
    );
    let listed = run(&["cask", "list"]);
    assert!(listed.status.success());
    assert_eq!(String::from_utf8(listed.stdout).unwrap(), "My Work\n");
    let listed_json = run(&["cask", "list", "--output", "json"]);
    assert!(listed_json.status.success());
    let items: serde_json::Value = serde_json::from_slice(&listed_json.stdout).unwrap();
    let id = items[0]["id"].as_str().unwrap();
    let shown = run(&["cask", "get", id]);
    assert!(shown.status.success());
    assert_eq!(
        String::from_utf8(shown.stdout).unwrap(),
        "My Work\nStatus: active\n"
    );
    let shown_json = run(&["--output", "json", "cask", "get", id]);
    assert!(shown_json.status.success());
    let item: serde_json::Value = serde_json::from_slice(&shown_json.stdout).unwrap();
    assert_eq!(item["name"], "My Work");
    assert_eq!(item["id"], id);
    fs::remove_dir_all(root).unwrap();
}
