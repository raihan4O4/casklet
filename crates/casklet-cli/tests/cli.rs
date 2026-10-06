use std::{fs, process::Command};

#[test]
fn archive_and_explicit_in_place_ownership_share_cleanup_semantics_across_processes() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("cli-ownership-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_casklet"))
            .args(args)
            .env("CASKLET_DEV_ROOT", &root)
            .output()
            .unwrap()
    };
    let created = run(&["--output", "json", "cask", "create", "In place"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let cask: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let id = cask["id"].as_str().unwrap();
    assert!(run(&["cask", "archive", id]).status.success());
    assert!(run(&["cask", "restore", id]).status.success());
    let source = root.join("external.txt");
    fs::write(&source, b"leave here").unwrap();
    let path = source.to_string_lossy();
    let refused = run(&["resource", "adopt", id, &path]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("OWNERSHIP_CONSENT"));
    assert!(
        run(&["resource", "adopt", id, &path, "--confirm-ownership"])
            .status
            .success()
    );
    let first = run(&["--output", "json", "cleanup", "create", id]);
    let second = run(&["--output", "json", "plan", "create", id]);
    assert!(first.status.success());
    assert!(second.status.success());
    let first: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    let second: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(first["id"], second["id"]);
    assert_eq!(first["cleanupOnly"], true);
    assert_eq!(first["items"][0]["action"], "TRASH");
    assert!(run(&["cask", "archive", id]).status.success());
    assert_eq!(fs::read(&source).unwrap(), b"leave here");
    fs::remove_dir_all(root).unwrap();
}

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
    let backup_output = run(&["--output", "json", "diagnostics", "backup"]);
    assert!(
        backup_output.status.success(),
        "{}",
        String::from_utf8_lossy(&backup_output.stderr)
    );
    let backup: serde_json::Value = serde_json::from_slice(&backup_output.stdout).unwrap();
    let backup_path = backup["path"].as_str().unwrap();
    assert!(std::path::Path::new(backup_path).starts_with(root.join("backups")));
    assert!(std::path::Path::new(backup_path).is_file());
    assert!(!run(&["cask", "remove", id]).status.success());
    assert!(run(&["cask", "remove", id, "--yes"]).status.success());
    assert!(!run(&["cask", "restore", id]).status.success());
    assert!(!run(&["cask", "get", id]).status.success());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_commands_keep_human_output_brief_and_require_mode() {
    let root = std::env::temp_dir().join(format!("casklet-cli-resource-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let binary = env!("CARGO_BIN_EXE_casklet");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .env("CASKLET_DEV_ROOT", &root)
            .output()
            .unwrap()
    };
    assert!(run(&["cask", "create", "My Work"]).status.success());
    let listed = run(&["--output", "json", "cask", "list"]);
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let id = items[0]["id"].as_str().unwrap();
    let source = root.join("notes.txt");
    fs::write(&source, b"notes").unwrap();
    let source_text = source.to_string_lossy().into_owned();
    let missing_mode = run(&["resource", "add", id, &source_text]);
    assert!(!missing_mode.status.success());
    assert!(source.exists());
    for mode in ["copy", "move"] {
        assert!(
            !run(&["resource", "add", id, &source_text, "--mode", mode])
                .status
                .success()
        );
        assert!(source.exists());
    }
    let added = run(&["resource", "add", id, &source_text, "--mode", "reference"]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    assert_eq!(
        String::from_utf8(added.stdout).unwrap(),
        "Referenced: notes.txt\n"
    );
    let resources = run(&["resource", "list", id]);
    assert_eq!(
        String::from_utf8(resources.stdout).unwrap(),
        "notes.txt (referenced)\n"
    );
    assert!(source.exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(feature = "reviewed-removal", feature = "alpha-cleanup"))]
#[test]
fn cleanup_requires_explicit_noninteractive_confirmation() {
    let root = std::env::temp_dir().join(format!("casklet-cli-cleanup-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let binary = env!("CARGO_BIN_EXE_casklet");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .env("CASKLET_DEV_ROOT", &root)
            .output()
            .unwrap()
    };
    assert!(run(&["cask", "create", "Cleanup"]).status.success());
    let listed = run(&["--output", "json", "cask", "list"]);
    let casks: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let cask_id = casks[0]["id"].as_str().unwrap();
    let source = root.join("source.txt");
    fs::write(&source, b"preserve source").unwrap();
    let source_text = source.to_string_lossy().into_owned();
    assert!(
        run(&[
            "resource",
            "add",
            cask_id,
            &source_text,
            "--mode",
            "reference"
        ])
        .status
        .success()
    );
    let created = run(&["--output", "json", "plan", "create", cask_id]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let plan: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let plan_id = plan["id"].as_str().unwrap();
    let hash = plan["planHash"].as_str().unwrap();
    assert!(
        run(&["plan", "approve", plan_id, "--hash", hash])
            .status
            .success()
    );
    let refused = run(&["plan", "execute", plan_id, "--hash", hash, "--no-input"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("CONFIRMATION_REQUIRED"));
    assert!(source.exists());
    assert!(!run(&["cask", "remove", cask_id, "--yes"]).status.success());
    let executed = run(&[
        "plan",
        "execute",
        plan_id,
        "--hash",
        hash,
        "--no-input",
        "--yes",
    ]);
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert!(source.exists());
    assert!(
        String::from_utf8(run(&["resource", "list", cask_id]).stdout)
            .unwrap()
            .is_empty()
    );
    assert!(run(&["cask", "remove", cask_id, "--yes"]).status.success());
    assert!(source.exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(all(feature = "alpha-cleanup", not(feature = "reviewed-removal")))]
#[test]
fn alpha_cleanup_supports_msi_but_does_not_enable_winget_uninstall() {
    let binary = env!("CARGO_BIN_EXE_casklet");
    assert!(
        Command::new(binary)
            .args(["msi", "execute", "--help"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new(binary)
            .args(["cleanup", "execute", "--help"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !Command::new(binary)
            .args(["winget", "execute", "--help"])
            .output()
            .unwrap()
            .status
            .success()
    );
}
