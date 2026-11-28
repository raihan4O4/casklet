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
    assert!(String::from_utf8_lossy(&refused.stderr).contains("ownership"));
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
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("casklet-cli-{}", std::process::id()));
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
    assert!(
        String::from_utf8(listed.stdout)
            .unwrap()
            .contains("My Work")
    );
    let listed_json = run(&["cask", "list", "--output", "json"]);
    assert!(listed_json.status.success());
    let items: serde_json::Value = serde_json::from_slice(&listed_json.stdout).unwrap();
    let id = items[0]["id"].as_str().unwrap();
    let shown = run(&["cask", "get", id]);
    assert!(shown.status.success());
    let overview = String::from_utf8(shown.stdout).unwrap();
    assert!(
        overview.contains("My Work")
            && overview.contains("Status: active")
            && overview.contains("Total items: 0")
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
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("casklet-cli-resource-{}", std::process::id()));
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
    let listing = String::from_utf8(resources.stdout).unwrap();
    assert!(listing.contains("notes.txt") && listing.contains("REFERENCE"));
    assert!(source.exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(feature = "reviewed-removal", feature = "alpha-cleanup"))]
#[test]
fn cleanup_requires_explicit_noninteractive_confirmation() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("casklet-cli-cleanup-{}", uuid::Uuid::new_v4()));
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
    assert!(String::from_utf8_lossy(&refused.stderr).contains("Pass --yes"));
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
            .contains("No entries.")
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

#[cfg(windows)]
#[test]
fn folder_contents_and_storage_reconcile_changes_across_cli_processes() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("cli-folder-sync-{}", uuid::Uuid::new_v4()));
    let folder = root.join("external/project");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("initial.txt"), b"one").unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_casklet"))
            .args(["--output", "json"])
            .args(args)
            .env("CASKLET_DEV_ROOT", &root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let cask = run(&["cask", "create", "Folder reconciliation"]);
    let id = cask["id"].as_str().unwrap();
    let resource = run(&[
        "resource",
        "adopt",
        id,
        &folder.to_string_lossy(),
        "--confirm-ownership",
    ]);
    fs::create_dir_all(folder.join("nested")).unwrap();
    fs::write(folder.join("nested/later.txt"), b"later").unwrap();
    fs::write(folder.join("initial.txt"), b"modified").unwrap();
    let contents = run(&["resource", "contents", resource["id"].as_str().unwrap()]);
    assert_eq!(contents["ownership"], "OWNED");
    assert_eq!(contents["totalEntries"], 3);
    let listed = run(&["resource", "get", resource["id"].as_str().unwrap()]);
    assert_eq!(listed["sizeBytes"], 13);
    let plan = run(&["cleanup", "create", id]);
    assert_eq!(plan["items"][0]["action"], "TRASH");
    assert_eq!(plan["items"][0]["expectedSizeBytes"], 13);
    assert_eq!(fs::read(folder.join("nested/later.txt")).unwrap(), b"later");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn presentation_modes_errors_and_narrow_unicode_inventory() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("cli-presentation-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_casklet"))
            .args(args)
            .env("CASKLET_DEV_ROOT", &root)
            .env("NO_COLOR", "1")
            .env("COLUMNS", "32")
            .output()
            .unwrap()
    };
    let created = run(&["--output", "json", "cask", "create", "研究 notes"]);
    assert!(created.status.success());
    let cask: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let id = cask["id"].as_str().unwrap();
    let listing = run(&["cask", "list"]);
    let listing = String::from_utf8(listing.stdout).unwrap();
    assert!(listing.contains("研究 notes"));
    assert!(!listing.contains(id));
    assert!(!listing.contains('\u{1b}'));
    for line in listing.lines() {
        assert!(unicode_width::UnicodeWidthStr::width(line) <= 32, "{line}");
    }
    let colored = run(&["--color", "always", "cask", "list"]);
    assert!(String::from_utf8_lossy(&colored.stdout).contains("\u{1b}["));
    let uncolored = run(&["--color", "never", "cask", "list"]);
    assert!(!String::from_utf8_lossy(&uncolored.stdout).contains('\u{1b}'));
    let overview = run(&["cask", "overview", id]);
    assert!(overview.status.success());
    assert!(String::from_utf8_lossy(&overview.stdout).contains("Total items: 0"));
    let details = run(&["--verbose", "--output", "json", "cask", "get", id]);
    let details: serde_json::Value = serde_json::from_slice(&details.stdout).unwrap();
    assert_eq!(details, cask);
    assert!(details["revision"].is_number());
    let missing = run(&["--output", "json", "resource", "get", "missing"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&missing.stderr).unwrap();
    assert!(error["code"].is_string() && error["correlationId"].is_string());
    let typo = run(&["cask", "lst"]);
    assert!(!typo.status.success());
    assert!(String::from_utf8_lossy(&typo.stderr).contains("list"));
    let status = run(&["--output", "json", "recovery", "retry-manifests"]);
    assert!(status.status.success());
    assert!(serde_json::from_slice::<serde_json::Value>(&status.stdout).is_ok());
    let capabilities = run(&["winget", "capabilities"]);
    assert!(capabilities.status.success());
    assert!(
        !String::from_utf8_lossy(&capabilities.stdout)
            .trim_start()
            .starts_with('{')
    );
    assert!(run(&["cask", "remove", id, "--yes"]).status.success());
    fs::remove_dir_all(root).unwrap();
}
