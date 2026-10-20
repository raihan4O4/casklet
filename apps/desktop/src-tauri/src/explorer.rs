//! Launch requests contain intent only. Existing shared-core commands validate every mutation.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::{collections::VecDeque, sync::Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    id: u64,
    batch_id: u64,
    batch_size: usize,
    path: String,
    cask_id: Option<String>,
    routed: bool,
    is_installer: bool,
    install_direct: bool,
}

#[derive(Debug, Default)]
pub struct Requests(Mutex<(u64, VecDeque<Request>)>);

impl Request {
    fn visible_in(&self, label: &str) -> bool {
        if self.routed {
            self.cask_id
                .as_ref()
                .is_some_and(|id| label == format!("cask-{id}"))
        } else {
            label == "main"
        }
    }
}

impl Requests {
    #[cfg(test)]
    fn push_many(&self, paths: Vec<String>, cask_id: Option<&str>) -> Result<(), String> {
        self.push_batch(paths, cask_id, false)
    }
    fn push_batch(
        &self,
        paths: Vec<String>,
        cask_id: Option<&str>,
        install_direct: bool,
    ) -> Result<(), String> {
        let mut queue = self
            .0
            .lock()
            .map_err(|_| "Explorer requests are unavailable")?;
        if paths.len() + queue.1.len() > 32 {
            return Err("Finish or cancel an existing Explorer request before adding more.".into());
        }
        let batch_id = queue.0 + 1;
        let batch_size = paths.len();
        for path in paths {
            queue.0 += 1;
            let id = queue.0;
            queue.1.push_back(Request {
                id,
                batch_id,
                batch_size,
                path,
                cask_id: cask_id.map(str::to_owned),
                routed: false,
                is_installer: false,
                install_direct,
            });
        }
        Ok(())
    }
    #[cfg(test)]
    fn push(&self, path: String, cask_id: Option<&str>) -> Result<(), String> {
        self.push_many(vec![path], cask_id)
    }
    fn pending(&self, label: &str) -> Result<Option<Request>, String> {
        let queue = self
            .0
            .lock()
            .map_err(|_| "Explorer requests are unavailable")?;
        Ok(queue
            .1
            .iter()
            .find(|request| request.visible_in(label))
            .map(|request| {
                let mut result = request.clone();
                result.batch_size = queue
                    .1
                    .iter()
                    .filter(|entry| entry.batch_id == request.batch_id)
                    .count();
                result
            }))
    }
    fn ack(&self, label: &str, id: u64) -> Result<(), String> {
        let mut queue = self
            .0
            .lock()
            .map_err(|_| "Explorer requests are unavailable")?;
        let batch = queue
            .1
            .iter()
            .find(|request| request.id == id && request.visible_in(label))
            .map(|request| request.batch_id);
        queue.1.retain(|request| {
            if label == "main" {
                Some(request.batch_id) != batch
            } else {
                request.id != id || !request.visible_in(label)
            }
        });
        Ok(())
    }
    fn requeue(&self, label: &str) {
        if let Ok(mut queue) = self.0.lock() {
            for entry in &mut queue.1 {
                if entry.routed && entry.visible_in(label) {
                    entry.routed = false;
                    entry.cask_id = None;
                }
            }
        }
    }
}

pub fn window_closed(app: &tauri::AppHandle, label: &str) {
    app.state::<Requests>().requeue(label);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Review,
    Owned,
    Reference,
    Install,
}
type ParsedLaunch = (Vec<String>, Option<String>, Action);

pub struct Runtime {
    pub jobs: AtomicUsize,
    pub ui: AtomicBool,
    pub ready: AtomicBool,
    initialized: AtomicBool,
    early: Mutex<Vec<Vec<String>>>,
}
impl Runtime {
    pub fn new(args: &[String]) -> Self {
        Self {
            jobs: AtomicUsize::new(0),
            ui: AtomicBool::new(!matches!(
                args.get(1).map(String::as_str),
                Some("--explorer-owned" | "--explorer-reference")
            )),
            ready: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            early: Mutex::new(Vec::new()),
        }
    }
}
fn finish_background(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let runtime = handle.state::<Runtime>();
        if runtime.jobs.load(Ordering::SeqCst) == 0 && !runtime.ui.load(Ordering::SeqCst) {
            handle.exit(0);
        }
    });
}
fn parse(args: &[String]) -> Result<Option<ParsedLaunch>, String> {
    if args.len() == 1 {
        return Ok(None);
    }
    if !(5..=36).contains(&args.len()) || args[3] != "--" {
        return Err(
            "This launch request is invalid. Right-click the item again in Explorer.".into(),
        );
    }
    let action = match args[1].as_str() {
        "--explorer" => Action::Review,
        "--explorer-owned" => Action::Owned,
        "--explorer-reference" => Action::Reference,
        "--explorer-install" => Action::Install,
        _ => return Err("Unknown Explorer action.".into()),
    };
    if (args[2] == "choose" && action != Action::Review)
        || (action == Action::Install && args.len() != 5)
    {
        return Err(
            "This launch request is invalid. Right-click the item again in Explorer.".into(),
        );
    }
    let id = &args[2];
    if id != "choose"
        && (id.is_empty()
            || id.len() > 128
            || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return Err("The destination Cask identity is invalid.".into());
    }
    for path in &args[4..] {
        if path.len() > 32760 || path.contains('\0') || !std::path::Path::new(path).is_absolute() {
            return Err("Choose an item with an absolute filesystem path.".into());
        }
        #[cfg(windows)]
        if !matches!(std::path::Path::new(path).components().next(), Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::UNC(_, _) | std::path::Prefix::VerbatimDisk(_) | std::path::Prefix::VerbatimUNC(_, _)))
        {
            return Err(
                "Select a file or folder on a disk or network share, rather than a Windows device."
                    .into(),
            );
        }
    }
    // Do not require existence here: a deleted/inaccessible source gets a visible,
    // retryable review error rather than disappearing before the window starts.
    Ok(Some((
        args[4..].to_vec(),
        (id != "choose").then(|| id.clone()),
        action,
    )))
}

pub fn initialized(app: &tauri::AppHandle, initial: &[String]) {
    let runtime = app.state::<Runtime>();
    let pending = {
        let mut early = runtime.early.lock().expect("launch queue poisoned");
        runtime.initialized.store(true, Ordering::SeqCst);
        std::mem::take(&mut *early)
    };
    receive(app, initial);
    for args in pending {
        receive(app, &args);
    }
}
pub fn receive(app: &tauri::AppHandle, args: &[String]) {
    let runtime = app.state::<Runtime>();
    {
        let mut early = runtime.early.lock().expect("launch queue poisoned");
        if !runtime.initialized.load(Ordering::SeqCst) {
            if early.len() < 32 {
                early.push(args.to_vec());
            } else {
                drop(early);
                show_error(app, "Casklet is starting. Wait a moment and try again.");
            }
            return;
        }
    }
    match parse(args) {
        Ok(Some((paths, Some(id), action @ (Action::Owned | Action::Reference)))) => {
            let runtime = app.state::<Runtime>();
            if runtime
                .jobs
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < 32).then_some(n + 1)
                })
                .is_err()
            {
                show_error(
                    app,
                    "Casklet is busy. Wait for existing Explorer operations to finish and try again.",
                );
                return;
            }
            let handle = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let result = direct_add(
                    &handle.state::<super::Service>(),
                    &id,
                    &paths,
                    action == Action::Owned,
                );
                let _ = handle.emit("casks-changed", ());
                match result {
                    Ok(message) => {
                        if handle
                            .notification()
                            .builder()
                            .title("Casklet — items added")
                            .body(&message)
                            .show()
                            .is_err()
                        {
                            handle
                                .dialog()
                                .message(message)
                                .title("Casklet — items added")
                                .blocking_show();
                        }
                    }
                    Err(problem) => show_error(&handle, &problem),
                }
                handle
                    .state::<Runtime>()
                    .jobs
                    .fetch_sub(1, Ordering::SeqCst);
                finish_background(&handle);
            });
        }
        Ok(parsed) => {
            app.state::<Runtime>().ui.store(true, Ordering::SeqCst);
            let result = if let Some((paths, cask_id, action)) = parsed {
                app.state::<Requests>().push_batch(
                    paths,
                    cask_id.as_deref(),
                    action == Action::Install,
                )
            } else {
                Ok(())
            };
            if let Err(problem) = result {
                show_error(app, &problem);
            }
            if app.state::<Runtime>().ready.load(Ordering::SeqCst)
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        Err(problem) => {
            show_error(app, &problem);
            finish_background(app);
        }
    }
}
fn show_error(app: &tauri::AppHandle, problem: &str) {
    app.dialog()
        .message(problem)
        .title("Casklet Explorer request")
        .kind(tauri_plugin_dialog::MessageDialogKind::Error)
        .blocking_show();
}

/// Each item commits through the existing core. Report partial batches honestly;
/// never retry successful entries implicitly or grant ownership to references.
fn direct_add(
    service: &super::Service,
    id: &str,
    paths: &[String],
    owned: bool,
) -> Result<String, String> {
    let cask = service
        .get(
            &casklet_contracts::GetCask {
                version: casklet_contracts::CONTRACT_VERSION,
                id: id.into(),
            },
            "explorer-direct",
        )
        .map_err(|e| e.message)?;
    if cask.state != "ACTIVE" {
        return Err(
            "This Cask is archived. Restore it in Casklet or choose another destination.".into(),
        );
    }
    let mut added = 0;
    let mut failures = Vec::new();
    for path in paths {
        let result = if owned {
            service.adopt_resource(
                &casklet_contracts::AdoptResource {
                    version: casklet_contracts::CONTRACT_VERSION,
                    cask_id: id.into(),
                    source_path: path.clone(),
                    category_key: "auto".into(),
                    confirm_ownership: true,
                },
                "explorer-direct-owned",
                &|_, _| {},
            )
        } else {
            service.add_user_resource_with_events(
                &casklet_contracts::AddResource {
                    version: casklet_contracts::CONTRACT_VERSION,
                    cask_id: id.into(),
                    source_path: path.clone(),
                    category_key: "auto".into(),
                    mode: casklet_contracts::AddMode::Reference,
                },
                "explorer-direct-reference",
                &|_, _| {},
                &|_| {},
            )
        };
        match result {
            Ok(_) => added += 1,
            Err(e) => failures.push(format!("{path}: {}", e.message)),
        }
    }
    if failures.is_empty() {
        Ok(format!(
            "Processed {added} selection(s) for {}. Existing items keep their ownership and category.",
            cask.name
        ))
    } else {
        Err(format!(
            "Added {added} of {} selected items to {}. The following items could not be confirmed. Inspect this Cask and Activity, check paths and permissions, and retry only items that are still missing.\n\n{}",
            paths.len(),
            cask.name,
            failures.join("\n")
        ))
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri extractors require owned arguments.
pub fn explorer_pending(
    window: tauri::WebviewWindow,
    requests: tauri::State<'_, Requests>,
) -> Result<Option<Request>, String> {
    requests.pending(window.label())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri extractors require owned arguments.
pub fn explorer_ack(
    window: tauri::WebviewWindow,
    requests: tauri::State<'_, Requests>,
    id: u64,
) -> Result<(), String> {
    requests.ack(window.label(), id)
}

#[tauri::command]
pub async fn explorer_route(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: u64,
    cask_id: String,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Choose the destination in the main window.".into());
    }
    let service = app.state::<super::Service>();
    let request = casklet_contracts::GetCask {
        version: casklet_contracts::CONTRACT_VERSION,
        id: cask_id.clone(),
    };
    let cask = service.get(&request, "explorer").map_err(|e| e.message)?;
    if cask.state != "ACTIVE" {
        return Err("This Cask is archived. Restore it or choose another Cask.".into());
    }
    let (batch, paths) = {
        let state = app.state::<Requests>();
        let queue = state
            .0
            .lock()
            .map_err(|_| "Explorer requests are unavailable")?;
        let batch = queue
            .1
            .iter()
            .find(|entry| entry.id == id && !entry.routed)
            .ok_or("This request has already been handled.")?
            .batch_id;
        (
            batch,
            queue
                .1
                .iter()
                .filter(|entry| entry.batch_id == batch && !entry.routed)
                .map(|entry| (entry.id, entry.path.clone()))
                .collect::<Vec<_>>(),
        )
    };
    // Inspect on a worker with no queue/database lock held (network shares can be slow).
    let inspected = tauri::async_runtime::spawn_blocking(move || {
        paths
            .into_iter()
            .map(|(id, path)| {
                let metadata = std::fs::metadata(&path).map_err(|e| {
                    format!(
                        "Cannot open {path}. Check that it still exists and you have access. {e}"
                    )
                })?;
                if !metadata.is_file() && !metadata.is_dir() {
                    return Err("Choose a regular file or folder.".to_string());
                }
                Ok((
                    id,
                    metadata.is_file()
                        && std::path::Path::new(&path)
                            .extension()
                            .is_some_and(|extension| {
                                extension.eq_ignore_ascii_case("exe")
                                    || extension.eq_ignore_ascii_case("msi")
                            }),
                ))
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .await
    .map_err(|e| e.to_string())??;
    {
        let state = app.state::<Requests>();
        let mut queue = state
            .0
            .lock()
            .map_err(|_| "Explorer requests are unavailable")?;
        if !queue.1.iter().any(|entry| entry.id == id && !entry.routed) {
            return Err("This request has already been handled.".into());
        }
        for entry in queue
            .1
            .iter_mut()
            .filter(|entry| entry.batch_id == batch && !entry.routed)
        {
            entry.is_installer = inspected
                .iter()
                .find(|(id, _)| *id == entry.id)
                .is_some_and(|(_, installer)| *installer);
            entry.cask_id = Some(cask_id.clone());
            entry.routed = true;
        }
    }
    let result =
        super::open_cask_window(app.clone(), service, app.state::<Mutex<()>>(), request).await;
    if let Err(problem) = result {
        if let Ok(mut queue) = app.state::<Requests>().0.lock() {
            for entry in queue.1.iter_mut().filter(|entry| entry.batch_id == batch) {
                entry.routed = false;
            }
        }
        return Err(problem.message);
    }
    let _ = app.emit_to(format!("cask-{cask_id}"), "explorer-requests", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Requests, parse};
    #[test]
    fn batch_capacity_and_cancellation_are_atomic() {
        let requests = Requests::default();
        requests.push_many(vec!["C:\\a".into(); 31], None).unwrap();
        assert!(requests.push_many(vec!["C:\\b".into(); 2], None).is_err());
        assert_eq!(requests.0.lock().unwrap().1.len(), 31);
        let first = requests.pending("main").unwrap().unwrap();
        assert_eq!(first.batch_size, 31);
        requests.ack("main", first.id).unwrap();
        assert!(requests.pending("main").unwrap().is_none());
        assert_eq!(
            parse(&args(&[
                "app",
                "--explorer",
                "choose",
                "--",
                "C:\\a",
                "C:\\b"
            ]))
            .unwrap()
            .unwrap()
            .0
            .len(),
            2
        );
    }
    #[test]
    #[cfg(windows)]
    fn native_shell_selection_abi_and_argument_encoding() {
        let output = std::process::Command::new(env!("CASKLET_EXPLORER_TEST_EXE"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "native Explorer contracts: {output:?}"
        );
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "Requires the separately installed QA package and seven active fixture Casks"]
    fn registered_native_shell() {
        let output = std::process::Command::new(env!("CASKLET_EXPLORER_TEST_EXE"))
            .arg("--registered")
            .output()
            .unwrap();
        println!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "registered native Explorer: {output:?}"
        );
    }
    #[test]
    fn queued_intent_is_scoped_bounded_and_survives_window_close() {
        let requests = Requests::default();
        for index in 0..32 {
            requests
                .push(format!("C:\\{index}.txt"), Some("a"))
                .unwrap();
        }
        assert!(requests.push("C:\\overflow.txt".into(), None).is_err());
        let first = requests.pending("main").unwrap().unwrap();
        requests.ack("cask-a", first.id).unwrap();
        assert_eq!(requests.pending("main").unwrap().unwrap().id, first.id);
        {
            let mut queue = requests.0.lock().unwrap();
            queue.1.front_mut().unwrap().routed = true;
        }
        assert!(requests.pending("cask-b").unwrap().is_none());
        assert_eq!(requests.pending("cask-a").unwrap().unwrap().id, first.id);
        requests.ack("main", first.id).unwrap();
        assert!(requests.pending("cask-a").unwrap().is_some());
        requests.requeue("cask-a");
        let restored = requests.pending("main").unwrap().unwrap();
        assert_eq!(restored.id, first.id);
        assert!(restored.cask_id.is_none());
        requests.ack("main", first.id).unwrap();
        requests.ack("main", first.id).unwrap();
        assert_ne!(requests.pending("main").unwrap().unwrap().id, first.id);
    }
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).into()).collect()
    }
    #[test]
    fn explicit_actions_are_validated_and_never_implicitly_install() {
        for (flag, action) in [
            ("--explorer-owned", super::Action::Owned),
            ("--explorer-reference", super::Action::Reference),
            ("--explorer-install", super::Action::Install),
        ] {
            let parsed = parse(&args(&[
                "app",
                flag,
                "cask-123",
                "--",
                "C:\\資料 & files\\test.exe",
            ]))
            .unwrap()
            .unwrap();
            assert_eq!(parsed.2, action);
            assert_eq!(parsed.1.as_deref(), Some("cask-123"));
            assert!(parse(&args(&["app", flag, "choose", "--", "C:\\a"])).is_err());
        }
        assert!(
            parse(&args(&[
                "app",
                "--explorer-install",
                "cask",
                "--",
                "C:\\a.exe",
                "C:\\b.exe"
            ]))
            .is_err()
        );
        assert!(
            !super::Runtime::new(&args(&["app", "--explorer-owned"]))
                .ui
                .load(std::sync::atomic::Ordering::SeqCst)
        );
        assert!(
            super::Runtime::new(&args(&["app", "--explorer-install"]))
                .ui
                .load(std::sync::atomic::Ordering::SeqCst)
        );
    }
    #[test]
    fn direct_core_intake_preserves_source_and_ownership_and_reports_partial_failure() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target")
            .join(format!(
                "direct-intake-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let service = super::super::Service {
            repository: casklet_infrastructure::SqliteCasks::open(root.join("state/db.sqlite"))
                .unwrap(),
            storage: casklet_infrastructure::LocalStorage::new(root.join("managed")),
        };
        let cask = service
            .create(
                &casklet_contracts::CreateCask {
                    version: 1,
                    name: "Direct actions".into(),
                },
                "test",
            )
            .unwrap();
        let owned = root.join("owned.exe");
        let reference = root.join("reference.msi");
        std::fs::write(&owned, b"not an executable").unwrap();
        std::fs::write(&reference, b"not an installer").unwrap();
        super::direct_add(
            &service,
            &cask.id,
            &[owned.to_string_lossy().into_owned()],
            true,
        )
        .unwrap();
        let result = super::direct_add(
            &service,
            &cask.id,
            &[
                reference.to_string_lossy().into_owned(),
                root.join("missing.txt").to_string_lossy().into_owned(),
            ],
            false,
        )
        .unwrap_err();
        assert!(result.contains("Added 1 of 2"), "{result}");
        let resources = service
            .list_resources(
                &casklet_contracts::ListResources {
                    version: 1,
                    cask_id: cask.id.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(resources.len(), 2);
        assert!(
            resources
                .iter()
                .any(|item| item.name == "owned.exe" && item.management == "MANAGED")
        );
        assert!(
            resources
                .iter()
                .any(|item| item.name == "reference.msi" && item.management == "REFERENCED")
        );
        assert_eq!(std::fs::read(owned).unwrap(), b"not an executable");
        assert_eq!(std::fs::read(reference).unwrap(), b"not an installer");
        assert!(super::direct_add(&service, "deleted-cask", &[], false).is_err());
        // Dedicated fixture retained for evidence; no user resources are deleted.
    }
    #[test]
    fn launch_contract_is_strict_and_preserves_unicode() {
        assert!(parse(&args(&["app"])).unwrap().is_none());
        let value = parse(&args(&[
            "app",
            "--explorer",
            "cask-123",
            "--",
            "C:\\資料 & files\\test.exe",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(value.0, ["C:\\資料 & files\\test.exe"]);
        assert_eq!(value.1.as_deref(), Some("cask-123"));
        for values in [
            vec!["app", "--explorer"],
            vec!["app", "--explorer", "bad\"id", "--", "C:\\a"],
            vec!["app", "--explorer", "choose", "--", "relative"],
            vec!["app", "--explorer", "choose", "--", "C:\\a", "relative"],
        ] {
            assert!(parse(&args(&values)).is_err());
        }
    }
}
