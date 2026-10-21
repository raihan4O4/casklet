//! Integration evidence for explicit ownership, independent cleanup, and reversible Cask lifecycle.
use super::*;
use casklet_application::resource::ResourceRepository;
use casklet_application::resource::ResourceStorage;
use casklet_contracts::{
    AddMode, AddResource, AdoptResource, CONTRACT_VERSION, CreateCask, CreateRemovalPlan,
    SetCaskArchived,
};

struct Fixture {
    root: PathBuf,
    service: CaskService<SqliteCasks, LocalStorage>,
    cask: casklet_contracts::CaskView,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("ownership-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("db/casklet.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Ownership".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("external/project.txt");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"user data").unwrap();
        Self {
            root,
            service,
            cask,
            source,
        }
    }
    fn adopt(
        &self,
        confirm: bool,
    ) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
        self.service.adopt_resource(
            &AdoptResource {
                version: CONTRACT_VERSION,
                cask_id: self.cask.id.clone(),
                source_path: self.source.to_string_lossy().into_owned(),
                category_key: "other".into(),
                confirm_ownership: confirm,
            },
            "test",
            &|_, _| {},
        )
    }
    fn plan(&self, permanent: bool) -> casklet_contracts::RemovalPlanView {
        self.service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: self.cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                    permanent,
                },
                "test",
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn consent_is_required_and_owned_data_remains_in_place_across_restart() {
    let f = Fixture::new();
    assert_eq!(f.adopt(false).unwrap_err().code, "OWNERSHIP_CONSENT");
    let owned = f.adopt(true).unwrap();
    assert_eq!(owned.management, "MANAGED");
    assert_eq!(owned.containment, "EXTERNAL");
    assert_eq!(owned.ownership, "OWNED");
    assert_eq!(owned.provenance, "USER_ADOPTED");
    assert_eq!(
        fs::canonicalize(&f.source).unwrap(),
        PathBuf::from(&owned.path)
    );
    assert_eq!(fs::read(&f.source).unwrap(), b"user data");
    let reopened = SqliteCasks::open(f.root.join("db/casklet.db")).unwrap();
    let grants: i64 = reopened
        .connection()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM resource_ownership_grants",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(grants, 1);
    assert_eq!(f.plan(false).items[0].action, "TRASH");
    assert_eq!(f.plan(true).items[0].action, "PRESERVE");
}

#[test]
fn equivalent_plans_reuse_ids_and_changed_or_missing_objects_are_preserved() {
    let f = Fixture::new();
    f.adopt(true).unwrap();
    let first = f.plan(false);
    let repeated = f.plan(false);
    assert_eq!(first.id, repeated.id);
    assert!(first.cleanup_only);
    assert!(f.source.exists());
    f.service
        .approve_removal_plan(
            &casklet_contracts::ApproveRemovalPlan {
                version: CONTRACT_VERSION,
                id: first.id.clone(),
                plan_hash: first.plan_hash.clone(),
            },
            "test",
        )
        .unwrap();
    let approved = f.plan(false);
    assert_eq!(approved.id, first.id);
    assert_eq!(approved.state, "APPROVED");
    fs::write(&f.source, b"changed by another application").unwrap();
    let changed = f.plan(false);
    assert_ne!(first.id, changed.id);
    assert_eq!(changed.items[0].action, "PRESERVE");
    fs::remove_file(&f.source).unwrap();
    assert_eq!(f.plan(false).items[0].action, "PRESERVE");
}

#[test]
fn overlapping_references_block_ownership_and_later_cleanup() {
    let f = Fixture::new();
    f.adopt(true).unwrap();
    let other = f
        .service
        .create(
            &CreateCask {
                version: CONTRACT_VERSION,
                name: "Another".into(),
            },
            "test",
        )
        .unwrap();
    f.service
        .add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: other.id,
                source_path: f.source.parent().unwrap().to_string_lossy().into_owned(),
                mode: AddMode::Reference,
                category_key: "other".into(),
            },
            "test",
            &|_, _| {},
        )
        .unwrap();
    let plan = f.plan(false);
    assert_eq!(plan.items[0].action, "DETACH");
    assert_eq!(plan.items[0].reason_code, "SHARED");
    assert_eq!(f.adopt(true).unwrap().ownership, "OWNED");
    assert!(f.source.exists());
}

#[test]
fn archive_and_restore_preserve_data_and_stale_revisions_fail() {
    let f = Fixture::new();
    let archived = f
        .service
        .set_cask_archived(
            &SetCaskArchived {
                version: CONTRACT_VERSION,
                id: f.cask.id.clone(),
                archived: true,
                expected_revision: f.cask.revision,
            },
            "test",
        )
        .unwrap();
    assert_eq!(archived.state, "ARCHIVED");
    assert!(f.source.exists());
    assert!(f.adopt(true).is_err());
    assert!(
        f.service
            .set_cask_archived(
                &SetCaskArchived {
                    version: CONTRACT_VERSION,
                    id: f.cask.id.clone(),
                    archived: false,
                    expected_revision: f.cask.revision
                },
                "test"
            )
            .is_err()
    );
    let restored = f
        .service
        .set_cask_archived(
            &SetCaskArchived {
                version: CONTRACT_VERSION,
                id: f.cask.id.clone(),
                archived: false,
                expected_revision: archived.revision,
            },
            "test",
        )
        .unwrap();
    assert_eq!(restored.state, "ACTIVE");
    f.adopt(true).unwrap();
    let populated = f
        .service
        .set_cask_archived(
            &SetCaskArchived {
                version: CONTRACT_VERSION,
                id: f.cask.id.clone(),
                archived: true,
                expected_revision: restored.revision,
            },
            "test",
        )
        .unwrap();
    assert_eq!(populated.state, "ARCHIVED");
    assert_eq!(fs::read(&f.source).unwrap(), b"user data");
}

#[test]
fn broad_managed_and_database_paths_cannot_be_adopted() {
    let f = Fixture::new();
    for path in [&f.root, &f.root.join("casks"), &f.root.join("db")] {
        assert!(
            f.service
                .adopt_resource(
                    &AdoptResource {
                        version: CONTRACT_VERSION,
                        cask_id: f.cask.id.clone(),
                        source_path: path.to_string_lossy().into_owned(),
                        category_key: "other".into(),
                        confirm_ownership: true
                    },
                    "test",
                    &|_, _| {}
                )
                .is_err()
        );
    }
    let observed = f
        .service
        .storage
        .inspect(&f.source.to_string_lossy(), &|_, _| {})
        .unwrap();
    assert!(f.service.storage.validate_owned_location(&observed).is_ok());
}

#[cfg(windows)]
#[test]
fn hard_links_and_directory_changes_revoke_external_cleanup_eligibility() {
    let f = Fixture::new();
    let link = f.root.join("external/also-needed.txt");
    fs::hard_link(&f.source, &link).unwrap();
    assert!(f.adopt(true).is_err());
    fs::remove_file(&link).unwrap();
    f.adopt(true).unwrap();
    fs::hard_link(&f.source, &link).unwrap();
    assert_eq!(f.plan(false).items[0].action, "PRESERVE");
    assert!(f.source.exists());
    assert!(link.exists());
}

#[test]
fn manifests_describe_external_ownership_but_recovery_never_restores_authority() {
    let f = Fixture::new();
    f.adopt(true).unwrap();
    let machine = f
        .service
        .repository
        .export_inventory(&f.cask.id, super::inventory::ExportProfile::Machine)
        .unwrap();
    assert_eq!(machine["resources"][0]["ownership"], "OWNED");
    assert_eq!(machine["resources"][0]["containment"], "EXTERNAL");
    let portable = f
        .service
        .repository
        .export_inventory(&f.cask.id, super::inventory::ExportProfile::Portable)
        .unwrap();
    assert!(portable["resources"][0].get("path").is_none());
    let recovered = SqliteCasks::open(f.root.join("recovered/db/casklet.db")).unwrap();
    let manifest = Path::new(&f.cask.managed_storage_root).join(".casklet/manifest.json");
    recovered
        .recover_manifest(&f.service.storage, &manifest)
        .unwrap();
    let resources = recovered.list_resources(&f.cask.id).unwrap();
    assert_eq!(resources[0].management, "REFERENCED");
    assert_eq!(resources[0].ownership, "REFERENCED");
    let grants: i64 = recovered
        .connection()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM resource_ownership_grants",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(grants, 0);
    assert_eq!(fs::read(&f.source).unwrap(), b"user data");
}

#[test]
fn migration_invalidates_legacy_drafts_and_keeps_historical_plans() {
    let f = Fixture::new();
    let db = f.root.join("legacy.db");
    let connection = Connection::open(&db).unwrap();
    for migration in [
        include_str!("../../../migrations/0001_casks.sql"),
        include_str!("../../../migrations/0002_resources.sql"),
        include_str!("../../../migrations/0003_operations.sql"),
        include_str!("../../../migrations/0004_removal_plans.sql"),
        include_str!("../../../migrations/0005_removal_execution.sql"),
        include_str!("../../../migrations/0006_permanent_disposal.sql"),
        include_str!("../../../migrations/0007_portable_installations.sql"),
        include_str!("../../../migrations/0008_windows_inventory.sql"),
        include_str!("../../../migrations/0009_winget_lifecycle.sql"),
        include_str!("../../../migrations/0010_installer_artifacts.sql"),
        include_str!("../../../migrations/0011_cask_metadata.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection.execute("INSERT INTO casks(id,name,storage_key,managed_storage_root,state,created_at_ms,updated_at_ms,revision) VALUES ('legacy','Legacy','legacy','unused','ACTIVE',1,1,0)",[]).unwrap();
    for (id, state) in [
        ("draft", "DRAFT"),
        ("approved", "APPROVED"),
        ("history", "COMPLETE"),
    ] {
        connection.execute("INSERT INTO removal_plans(id,cask_id,scope,state,based_on_cask_revision,plan_hash,created_at_ms,expires_at_ms) VALUES (?1,'legacy','CASK',?2,0,'old',1,2)",params![id,state]).unwrap();
    }
    connection.pragma_update(None, "user_version", 11).unwrap();
    drop(connection);
    let migrated = SqliteCasks::open(db).unwrap();
    let connection = migrated.connection().unwrap();
    for id in ["draft", "approved"] {
        let state: String = connection
            .query_row("SELECT state FROM removal_plans WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(state, "INVALIDATED");
    }
    let state: String = connection
        .query_row(
            "SELECT state FROM removal_plans WHERE id='history'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "COMPLETE");
    assert_eq!(migrated.get("legacy").unwrap().unwrap().name, "Legacy");
}

#[test]
fn cleanup_cannot_start_during_another_casks_overlapping_intake() {
    use casklet_application::removal::RemovalRepository;
    use std::sync::atomic::{AtomicBool, Ordering};
    let f = Fixture::new();
    f.adopt(true).unwrap();
    let plan = f.plan(false);
    f.service
        .approve_removal_plan(
            &casklet_contracts::ApproveRemovalPlan {
                version: CONTRACT_VERSION,
                id: plan.id.clone(),
                plan_hash: plan.plan_hash.clone(),
            },
            "test",
        )
        .unwrap();
    let other = f
        .service
        .create(
            &CreateCask {
                version: CONTRACT_VERSION,
                name: "Copy consumer".into(),
            },
            "test",
        )
        .unwrap();
    let checked = AtomicBool::new(false);
    f.service
        .add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: other.id,
                source_path: f.source.to_string_lossy().into_owned(),
                mode: AddMode::Copy,
                category_key: "other".into(),
            },
            "test",
            &|_, _| {
                let locks: i64 = f
                    .service
                    .repository
                    .connection()
                    .unwrap()
                    .query_row("SELECT COUNT(*) FROM operation_locks", [], |row| row.get(0))
                    .unwrap();
                if locks > 0 && !checked.swap(true, Ordering::SeqCst) {
                    let result = f.service.repository.begin_execution(
                        &plan.id,
                        &plan.plan_hash,
                        &uuid::Uuid::new_v4().to_string(),
                        plan.created_at_ms + 1,
                    );
                    assert!(
                        result
                            .unwrap_err()
                            .contains("overlaps an active or unrecovered intake")
                    );
                    assert_eq!(fs::read(&f.source).unwrap(), b"user data");
                }
            },
        )
        .unwrap();
    assert!(checked.load(Ordering::SeqCst));
    assert_eq!(
        f.service
            .repository
            .get_plan(&plan.id)
            .unwrap()
            .unwrap()
            .state,
        "APPROVED"
    );
}

fn remove_request(f: &Fixture, confirm: bool) -> casklet_contracts::RemoveCask {
    casklet_contracts::RemoveCask {
        version: CONTRACT_VERSION,
        id: f.cask.id.clone(),
        expected_revision: f.cask.revision,
        confirm,
    }
}
#[test]
fn permanent_empty_removal_requires_confirmation_and_cannot_restore_or_recover() {
    let f = Fixture::new();
    let mut request = remove_request(&f, false);
    assert_eq!(
        f.service.remove_cask(&request, "test").unwrap_err().code,
        "CONFIRMATION_REQUIRED"
    );
    request.confirm = true;
    request.expected_revision += 1;
    assert_eq!(
        f.service.remove_cask(&request, "test").unwrap_err().code,
        "CASK_NOT_REMOVABLE"
    );
    request.expected_revision = f.cask.revision;
    assert!(f.service.remove_cask(&request, "test").unwrap().is_none());
    assert!(!Path::new(&f.cask.managed_storage_root).exists());
    assert!(
        f.root
            .join("retired-casks")
            .join(&f.cask.id)
            .join("manifest.json")
            .exists()
    );
    assert!(f.service.repository.get(&f.cask.id).unwrap().is_none());
    assert!(f.service.repository.list().unwrap().is_empty());
    assert_eq!(
        f.service
            .set_cask_archived(
                &SetCaskArchived {
                    version: CONTRACT_VERSION,
                    id: f.cask.id.clone(),
                    archived: false,
                    expected_revision: f.cask.revision + 1
                },
                "test"
            )
            .unwrap_err()
            .code,
        "NOT_FOUND"
    );
    let manifest = Path::new(&f.cask.managed_storage_root).join(".casklet/manifest.json");
    assert!(
        f.service
            .repository
            .recover_manifest(&f.service.storage, &manifest)
            .is_err()
    );
    let reopened = SqliteCasks::open(f.root.join("db/casklet.db")).unwrap();
    assert!(reopened.get(&f.cask.id).unwrap().is_none());
    let delayed = casklet_domain::resource::Resource::new(
        f.cask.id.clone(),
        "Delayed provider intake".into(),
        f.source.to_string_lossy().into_owned(),
        casklet_domain::resource::ResourceKind::File,
        casklet_domain::resource::IntakeMode::Copy,
        "other".into(),
        Some(9),
        0,
        2,
    )
    .unwrap();
    assert!(
        f.service
            .repository
            .begin_intake(
                "delayed",
                &delayed,
                casklet_domain::resource::IntakeMode::Copy,
                &f.source.to_string_lossy(),
                "staging",
                "destination"
            )
            .is_err()
    );
    assert_eq!(fs::read(&f.source).unwrap(), b"user data");
}

#[test]
fn cask_removal_preserves_unknown_files_and_recovery_bytes_with_a_durable_warning() {
    for relative in ["content/unknown.txt", ".casklet/quarantine/uncertain.txt"] {
        let f = Fixture::new();
        let unknown = Path::new(&f.cask.managed_storage_root).join(relative);
        fs::create_dir_all(unknown.parent().unwrap()).unwrap();
        fs::write(&unknown, b"preserve me").unwrap();
        let warning = f
            .service
            .remove_cask(&remove_request(&f, true), "test")
            .unwrap()
            .unwrap();
        assert!(warning.contains("preserved"));
        assert_eq!(fs::read(&unknown).unwrap(), b"preserve me");
        assert!(f.service.repository.get(&f.cask.id).unwrap().is_none());
        let persisted: String = f
            .service
            .repository
            .connection()
            .unwrap()
            .query_row(
                "SELECT storage_warning FROM cask_removals WHERE cask_id=?1",
                [&f.cask.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted, warning);
    }
}
#[test]
fn remaining_resources_or_applications_are_not_empty() {
    let f = Fixture::new();
    f.adopt(true).unwrap();
    assert!(
        !f.service
            .repository
            .removal_status(&f.cask.id)
            .unwrap()
            .can_remove
    );
    assert_eq!(
        f.service
            .remove_cask(&remove_request(&f, true), "test")
            .unwrap_err()
            .code,
        "CASK_NOT_REMOVABLE"
    );
    let g = Fixture::new();
    let connection = g.service.repository.connection().unwrap();
    connection.execute("INSERT INTO applications(id,canonical_name,created_at_ms) VALUES ('app','Application',1)", []).unwrap();
    connection.execute("INSERT INTO windows_installations(id,application_id,source_key,scope,display_name,evidence_json,evidence_sha256,state,first_seen_at_ms,last_seen_at_ms,last_verified_at_ms) VALUES ('install','app','source','USER','Application','{}','hash','MISSING',1,1,1)", []).unwrap();
    connection.execute("INSERT INTO windows_cask_associations(id,cask_id,installation_id,created_at_ms) VALUES ('association',?1,'install',1)", [&g.cask.id]).unwrap();
    let status = g.service.repository.removal_status(&g.cask.id).unwrap();
    assert_eq!(status.resource_count, 0);
    assert_eq!(status.application_count, 1);
    assert!(!status.can_remove);
    assert_eq!(
        g.service
            .remove_cask(&remove_request(&g, true), "test")
            .unwrap_err()
            .code,
        "CASK_NOT_REMOVABLE"
    );
}
#[test]
fn unfinished_work_blocks_removal_without_relying_only_on_locks() {
    let f = Fixture::new();
    f.service.repository.connection().unwrap().execute("INSERT INTO operations(id,cask_id,type,state,current_step_code,started_at_ms,updated_at_ms) VALUES ('pending',?1,'RESOURCE_INTAKE','RECOVERY_REQUIRED','RECONCILE',1,1)", [&f.cask.id]).unwrap();
    assert!(
        !f.service
            .repository
            .removal_status(&f.cask.id)
            .unwrap()
            .can_remove
    );
    assert_eq!(
        f.service
            .remove_cask(&remove_request(&f, true), "test")
            .unwrap_err()
            .code,
        "CASK_NOT_REMOVABLE"
    );
}
#[test]
fn successful_reference_cleanup_allows_separate_permanent_cask_removal() {
    let f = Fixture::new();
    f.service
        .add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: f.cask.id.clone(),
                source_path: f.source.to_string_lossy().into_owned(),
                mode: AddMode::Reference,
                category_key: "other".into(),
            },
            "test",
            &|_, _| {},
        )
        .unwrap();
    let plan = f.plan(false);
    assert_eq!(plan.items[0].action, "DETACH");
    assert!(
        f.service
            .remove_cask(&remove_request(&f, true), "test")
            .is_err()
    );
    f.service
        .approve_removal_plan(
            &casklet_contracts::ApproveRemovalPlan {
                version: CONTRACT_VERSION,
                id: plan.id.clone(),
                plan_hash: plan.plan_hash.clone(),
            },
            "test",
        )
        .unwrap();
    let complete = f
        .service
        .execute_removal_plan(
            &casklet_contracts::ExecuteRemovalPlan {
                version: CONTRACT_VERSION,
                id: plan.id,
                plan_hash: plan.plan_hash,
                confirm_permanent: false,
            },
            "test",
        )
        .unwrap();
    assert_eq!(complete.state, "COMPLETE");
    assert!(
        f.service
            .repository
            .removal_status(&f.cask.id)
            .unwrap()
            .can_remove
    );
    f.service
        .remove_cask(&remove_request(&f, true), "test")
        .unwrap();
    assert_eq!(fs::read(&f.source).unwrap(), b"user data");
}
#[test]
fn user_copy_and_move_are_disabled_while_legacy_data_stays_supported() {
    let f = Fixture::new();
    for mode in [AddMode::Copy, AddMode::Move] {
        let request = AddResource {
            version: CONTRACT_VERSION,
            cask_id: f.cask.id.clone(),
            source_path: f.source.to_string_lossy().into_owned(),
            mode,
            category_key: "other".into(),
        };
        assert_eq!(
            f.service
                .add_user_resource_with_events(&request, "test", &|_, _| {}, &|_| {})
                .unwrap_err()
                .code,
            "INTAKE_MODE_DISABLED"
        );
        assert_eq!(fs::read(&f.source).unwrap(), b"user data");
    }
    let legacy = f
        .service
        .add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: f.cask.id.clone(),
                source_path: f.source.to_string_lossy().into_owned(),
                mode: AddMode::Copy,
                category_key: "other".into(),
            },
            "test",
            &|_, _| {},
        )
        .unwrap();
    assert_eq!(legacy.management, "MANAGED");
    assert_eq!(f.plan(false).items[0].action, "TRASH");
}

#[test]
fn cleanup_history_survives_restart_and_excludes_removed_casks() {
    use casklet_contracts::{ListRemovalPlans, RemoveCask};
    let f = Fixture::new();
    f.adopt(true).unwrap();
    let plan = f.plan(false);
    let reopened = CaskService {
        repository: SqliteCasks::open(f.root.join("db/casklet.db")).unwrap(),
        storage: LocalStorage::new(f.root.join("casks")),
    };
    let query = ListRemovalPlans {
        version: CONTRACT_VERSION,
        cask_id: Some(f.cask.id.clone()),
    };
    let history = reopened.list_removal_plans(&query, "test").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, plan.id);
    assert_eq!(history[0].plan_hash, plan.plan_hash);
    assert_eq!(
        reopened
            .list_removal_plans(
                &ListRemovalPlans {
                    version: 999,
                    cask_id: None
                },
                "test"
            )
            .unwrap_err()
            .code,
        "UNSUPPORTED_VERSION"
    );
    let empty = reopened
        .create(
            &CreateCask {
                version: CONTRACT_VERSION,
                name: "Empty".into(),
            },
            "test",
        )
        .unwrap();
    reopened
        .remove_cask(
            &RemoveCask {
                version: CONTRACT_VERSION,
                id: empty.id.clone(),
                expected_revision: empty.revision,
                confirm: true,
            },
            "test",
        )
        .unwrap();
    assert!(
        reopened
            .list_removal_plans(
                &ListRemovalPlans {
                    version: CONTRACT_VERSION,
                    cask_id: Some(empty.id)
                },
                "test"
            )
            .unwrap()
            .is_empty()
    );
}

#[test]
fn repeat_intake_reuses_identity_and_explicit_ownership_promotes_reference() {
    for kind in ["file.txt", "setup.exe", "setup.msi", "folder"] {
        let mut f = Fixture::new();
        f.source = f.root.join("external").join(kind);
        if kind == "folder" {
            fs::create_dir_all(&f.source).unwrap();
            fs::write(f.source.join("content.txt"), b"user data").unwrap();
        } else {
            fs::write(&f.source, b"user data").unwrap();
        }
        let request = AddResource {
            version: CONTRACT_VERSION,
            cask_id: f.cask.id.clone(),
            source_path: f.source.to_string_lossy().into_owned(),
            mode: AddMode::Reference,
            category_key: "other".into(),
        };
        let reference = f
            .service
            .add_resource(&request, "test", &|_, _| {})
            .unwrap();
        for _ in 0..3 {
            let again = f
                .service
                .add_resource(&request, "test", &|_, _| {})
                .unwrap();
            assert_eq!(again.id, reference.id);
        }
        let resources = f.service.repository.list_resources(&f.cask.id).unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].size_bytes, (kind != "folder").then_some(9));
        let summary_request = casklet_contracts::GetStorageSummary {
            version: CONTRACT_VERSION,
            cask_id: f.cask.id.clone(),
        };
        let summary = f.service.storage_summary(&summary_request, "test").unwrap();
        assert_eq!(summary.referenced_logical_bytes, 9);
        assert_eq!(summary.managed_logical_bytes, 0);
        assert_eq!(f.adopt(false).unwrap().ownership, "REFERENCED");
        assert_eq!(f.adopt(true).unwrap().ownership, "REFERENCED");
        let policy = casklet_contracts::SetResourceOwnership {
            version: CONTRACT_VERSION,
            cask_id: f.cask.id.clone(),
            resource_id: reference.id.clone(),
            expected_ownership: "REFERENCED".into(),
            owned: true,
            confirm_ownership: false,
        };
        assert_eq!(
            f.service
                .set_resource_ownership(&policy, "test")
                .unwrap_err()
                .code,
            "OWNERSHIP_CONSENT"
        );
        let owned = f
            .service
            .set_resource_ownership(
                &casklet_contracts::SetResourceOwnership {
                    confirm_ownership: true,
                    ..policy
                },
                "test",
            )
            .unwrap();
        assert_eq!(owned.id, reference.id);
        assert_eq!(owned.ownership, "OWNED");
        assert_eq!(f.adopt(true).unwrap().id, owned.id);
        let again = f
            .service
            .add_resource(&request, "test", &|_, _| {})
            .unwrap();
        assert_eq!(again.id, owned.id);
        assert_eq!(again.ownership, "OWNED");
        let summary = f.service.storage_summary(&summary_request, "test").unwrap();
        assert_eq!(summary.referenced_logical_bytes, 0);
        assert_eq!(summary.managed_logical_bytes, 9);
        assert_eq!(
            f.service
                .repository
                .list_resources(&f.cask.id)
                .unwrap()
                .len(),
            1
        );
        let connection = f.service.repository.connection().unwrap();
        let grants: i64 = connection
            .query_row("SELECT COUNT(*) FROM resource_ownership_grants", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(grants, 1);
        let installations: i64 = connection
            .query_row("SELECT COUNT(*) FROM installer_artifacts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(installations, 0);
        assert!(f.source.exists());
        assert_eq!(f.plan(false).items[0].action, "TRASH");
    }
}

#[test]
fn reference_promotion_preserves_other_cask_dependencies() {
    let f = Fixture::new();
    let other = f
        .service
        .create(
            &CreateCask {
                version: CONTRACT_VERSION,
                name: "Other".into(),
            },
            "test",
        )
        .unwrap();
    for cask_id in [&f.cask.id, &other.id] {
        f.service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask_id.clone(),
                    source_path: f.source.to_string_lossy().into_owned(),
                    mode: AddMode::Reference,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
    }
    assert_eq!(f.adopt(true).unwrap().ownership, "REFERENCED");
    let item = f
        .service
        .repository
        .list_resources(&f.cask.id)
        .unwrap()
        .remove(0);
    assert_eq!(
        f.service
            .set_resource_ownership(
                &casklet_contracts::SetResourceOwnership {
                    version: CONTRACT_VERSION,
                    cask_id: f.cask.id.clone(),
                    resource_id: item.id,
                    expected_ownership: item.ownership,
                    owned: true,
                    confirm_ownership: true
                },
                "test"
            )
            .unwrap_err()
            .code,
        "OWNERSHIP_CONFLICT"
    );
    for cask_id in [&f.cask.id, &other.id] {
        let items = f.service.repository.list_resources(cask_id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].ownership, "REFERENCED");
    }
    assert!(f.source.exists());
}

#[test]
fn legacy_duplicate_references_are_retired_when_readding_or_promoting() {
    for promote in [false, true] {
        let f = Fixture::new();
        let request = AddResource {
            version: CONTRACT_VERSION,
            cask_id: f.cask.id.clone(),
            source_path: f.source.to_string_lossy().into_owned(),
            mode: AddMode::Reference,
            category_key: "other".into(),
        };
        let first = f
            .service
            .add_resource(&request, "test", &|_, _| {})
            .unwrap();
        let connection = f.service.repository.connection().unwrap();
        connection.execute("INSERT INTO resources SELECT 'legacy-duplicate',kind,display_name,management,provenance,availability,size_bytes,size_measurement,unknown_entries,created_at_ms,revision FROM resources WHERE id=?1", [&first.id]).unwrap();
        connection.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms) SELECT 'duplicate-location','legacy-duplicate',path_display,path_key,containment,1,first_seen_at_ms FROM resource_locations WHERE resource_id=?1", [&first.id]).unwrap();
        connection.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) SELECT 'duplicate-association',cask_id,'legacy-duplicate',category_key,relationship,removal_policy,added_at_ms FROM resource_associations WHERE resource_id=?1", [&first.id]).unwrap();
        if promote {
            f.adopt(true).unwrap();
        } else {
            f.service
                .add_resource(&request, "test", &|_, _| {})
                .unwrap();
        }
        assert_eq!(
            f.service
                .repository
                .list_resources(&f.cask.id)
                .unwrap()
                .len(),
            1
        );
        assert!(f.source.exists());
    }
}

#[test]
fn concurrent_reference_requests_register_one_item() {
    let f = Fixture::new();
    let barrier = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    f.service
                        .add_resource(
                            &AddResource {
                                version: CONTRACT_VERSION,
                                cask_id: f.cask.id.clone(),
                                source_path: f.source.to_string_lossy().into_owned(),
                                mode: AddMode::Reference,
                                category_key: "other".into(),
                            },
                            "test",
                            &|_, _| {},
                        )
                        .unwrap()
                        .id
                })
            })
            .collect();
        let ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(ids.iter().all(|id| id == &ids[0]));
    });
    assert_eq!(
        f.service
            .repository
            .list_resources(&f.cask.id)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn explicit_policy_changes_preserve_bytes_and_invalidate_old_cleanup() {
    for folder in [false, true] {
        let mut f = Fixture::new();
        if folder {
            f.source = f.root.join("external/folder");
            fs::create_dir_all(&f.source).unwrap();
            fs::write(f.source.join("nested.txt"), b"user data").unwrap();
        }
        let owned = f.adopt(true).unwrap();
        let old_plan = f.plan(false);
        let request = casklet_contracts::SetResourceOwnership {
            version: CONTRACT_VERSION,
            cask_id: f.cask.id.clone(),
            resource_id: owned.id.clone(),
            expected_ownership: "OWNED".into(),
            owned: false,
            confirm_ownership: false,
        };
        let reference = f.service.set_resource_ownership(&request, "test").unwrap();
        assert_eq!(reference.id, owned.id);
        assert_eq!(reference.ownership, "REFERENCED");
        assert_eq!(f.adopt(true).unwrap().ownership, "REFERENCED");
        assert_eq!(f.plan(false).items[0].action, "DETACH");
        assert!(f.service.set_resource_ownership(&request, "test").is_err());
        let approval = casklet_contracts::ApproveRemovalPlan {
            version: CONTRACT_VERSION,
            id: old_plan.id,
            plan_hash: old_plan.plan_hash,
        };
        assert!(f.service.approve_removal_plan(&approval, "test").is_err());
        if folder {
            let contents = f
                .service
                .resource_contents(
                    &casklet_contracts::GetResource {
                        version: CONTRACT_VERSION,
                        id: owned.id.clone(),
                    },
                    true,
                    "test",
                )
                .unwrap();
            assert_eq!(contents.ownership, "REFERENCED");
        }
        let promoted = f
            .service
            .set_resource_ownership(
                &casklet_contracts::SetResourceOwnership {
                    expected_ownership: "REFERENCED".into(),
                    owned: true,
                    confirm_ownership: true,
                    ..request
                },
                "test",
            )
            .unwrap();
        assert_eq!(promoted.id, owned.id);
        assert_eq!(promoted.ownership, "OWNED");
        assert_eq!(f.plan(false).items[0].action, "TRASH");
        assert_eq!(
            fs::read(if folder {
                f.source.join("nested.txt")
            } else {
                f.source.clone()
            })
            .unwrap(),
            b"user data"
        );
    }
}

#[test]
fn equivalent_file_objects_do_not_duplicate_or_change_policy() {
    let f = Fixture::new();
    let link = f.root.join("external/different name.txt");
    fs::hard_link(&f.source, &link).unwrap();
    let add = |source: &Path| {
        f.service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: f.cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Reference,
                    category_key: "documents".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap()
    };
    let first = add(&f.source);
    assert_eq!(add(&link).id, first.id);
    assert_eq!(
        add(&f.source.parent().unwrap().join("./project.txt")).id,
        first.id
    );
    assert_eq!(f.adopt(true).unwrap().ownership, "REFERENCED");
    assert_eq!(
        f.service
            .repository
            .list_resources(&f.cask.id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(first.category_key, "documents");
}

#[test]
fn automatic_categories_are_shared_and_retries_preserve_manual_category() {
    for (name, expected) in [
        ("installer.EXE", "applications"),
        ("setup.msi", "applications"),
        ("book.PDF", "documents"),
        ("photo.JPG", "images"),
        ("film.MKV", "videos"),
        ("music.FLAC", "audio"),
        ("source.tar.gz", "archives"),
        ("unknown.xyz", "other"),
        ("exe", "other"),
    ] {
        let mut f = Fixture::new();
        f.source = f.root.join("external").join(name);
        fs::write(&f.source, b"fixture").unwrap();
        let request = AddResource {
            version: CONTRACT_VERSION,
            cask_id: f.cask.id.clone(),
            source_path: f.source.to_string_lossy().into_owned(),
            mode: AddMode::Reference,
            category_key: "auto".into(),
        };
        let item = f
            .service
            .add_resource(&request, "test", &|_, _| {})
            .unwrap();
        assert_eq!(item.category_key, expected);
        f.service
            .categorize_resource(
                &casklet_contracts::CategorizeResource {
                    version: CONTRACT_VERSION,
                    resource_id: item.id.clone(),
                    category_key: "projects".into(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(
            f.service
                .add_resource(&request, "test", &|_, _| {})
                .unwrap()
                .category_key,
            "projects"
        );
        assert_eq!(f.adopt(true).unwrap().category_key, "projects");
    }
}

#[cfg(feature = "reviewed-removal")]
#[test]
fn windows_recycle_restore_preserves_original_name_location_and_contents() {
    use std::os::windows::process::CommandExt;
    for name in [
        "Original name.txt",
        "Unicode café 数据.tar.gz",
        "Folder café 数据",
    ] {
        let mut f = Fixture::new();
        f.source = f.root.join("external").join(name);
        let folder = name.starts_with("Folder");
        if folder {
            fs::create_dir_all(f.source.join("nested")).unwrap();
            fs::write(f.source.join("nested/document.txt"), b"restore me").unwrap();
        } else {
            fs::write(&f.source, b"restore me").unwrap();
        }
        let owned = f.adopt(true).unwrap();
        let plan = f.plan(false);
        f.service
            .approve_removal_plan(
                &casklet_contracts::ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
        let result = f
            .service
            .execute_removal_plan(
                &casklet_contracts::ExecuteRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id,
                    plan_hash: plan.plan_hash,
                    confirm_permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(result.state, "COMPLETE");
        assert!(!f.source.exists());
        // Use Windows' native Restore verb. The trash crate's list() exposes a
        // display name (without known extensions), so its restore_all() is not
        // a faithful test of Windows Restore when extensions are hidden.
        let restore = std::process::Command::new(PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe"))
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", r"
$ErrorActionPreference='Stop'
$shell=New-Object -ComObject Shell.Application
$items=@($shell.Namespace(10).Items() | Where-Object { $_.ExtendedProperty('System.Recycle.DeletedFrom') -eq $env:CASKLET_RESTORE_PARENT })
if ($items.Count -ne 1) { throw 'Expected exactly one generated fixture in its original parent' }
$items[0].InvokeVerb('undelete')
$clock=[Diagnostics.Stopwatch]::StartNew()
while (-not (Test-Path -LiteralPath $env:CASKLET_RESTORE_PATH)) { if ($clock.Elapsed.TotalSeconds -gt 10) { throw 'Native restore did not complete' }; Start-Sleep -Milliseconds 50 }
"])
            .env("CASKLET_RESTORE_PARENT", fs::canonicalize(f.source.parent().unwrap()).unwrap().to_string_lossy().trim_start_matches(r"\\?\"))
            .env("CASKLET_RESTORE_PATH", &f.source)
            .creation_flags(0x0800_0000)
            .output().unwrap();
        assert!(
            restore.status.success(),
            "{}",
            String::from_utf8_lossy(&restore.stderr)
        );
        assert_eq!(
            fs::read(if folder {
                f.source.join("nested/document.txt")
            } else {
                f.source.clone()
            })
            .unwrap(),
            b"restore me"
        );
        assert!(
            f.service
                .repository
                .list_resources(&f.cask.id)
                .unwrap()
                .is_empty()
        );
        assert!(
            f.service
                .repository
                .get_resource(&owned.id)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn internal_allocations_can_change_policy_without_relocation() {
    let f = Fixture::new();
    let item = f
        .service
        .add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: f.cask.id.clone(),
                source_path: f.source.to_string_lossy().into_owned(),
                mode: AddMode::Copy,
                category_key: "documents".into(),
            },
            "test",
            &|_, _| {},
        )
        .unwrap();
    let request = casklet_contracts::SetResourceOwnership {
        version: CONTRACT_VERSION,
        cask_id: f.cask.id.clone(),
        resource_id: item.id.clone(),
        expected_ownership: "OWNED".into(),
        owned: false,
        confirm_ownership: false,
    };
    let reference = f.service.set_resource_ownership(&request, "test").unwrap();
    assert_eq!(reference.ownership, "REFERENCED");
    assert_eq!(reference.path, item.path);
    assert_eq!(f.plan(false).items[0].action, "DETACH");
    let renewed = f
        .service
        .set_resource_ownership(
            &casklet_contracts::SetResourceOwnership {
                expected_ownership: "REFERENCED".into(),
                owned: true,
                confirm_ownership: true,
                ..request
            },
            "test",
        )
        .unwrap();
    assert_eq!(renewed.id, item.id);
    assert_eq!(renewed.path, item.path);
    assert_eq!(f.plan(false).items[0].action, "TRASH");
    assert_eq!(fs::read(&item.path).unwrap(), b"user data");
}

#[cfg(feature = "reviewed-removal")]
#[test]
#[allow(clippy::too_many_lines)] // One restart/concurrency lifecycle fixture.
fn whole_cask_intent_survives_restart_and_finalizes_once_without_replaying_files() {
    use casklet_contracts::{CreateCategoryCleanup, ExecuteCategoryCleanup};
    let f = Fixture::new();
    f.service
        .add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: f.cask.id.clone(),
                source_path: f.source.to_string_lossy().into_owned(),
                mode: AddMode::Reference,
                category_key: "documents".into(),
            },
            "test",
            &|_, _| {},
        )
        .unwrap();
    let plan = f
        .service
        .create_category_cleanup(
            &CreateCategoryCleanup {
                scope: CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: f.cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                    permanent: false,
                },
                installation_id: None,
            },
            "test",
        )
        .unwrap();
    assert!(plan.cask_removal_id.is_some());
    let mut forged = plan.clone();
    forged.resources = None;
    assert_eq!(
        f.service
            .get_category_cleanup(&forged, "test")
            .unwrap_err()
            .code,
        "INVALID_PLAN"
    );
    let approved = f.service.approve_category_cleanup(&plan, "test").unwrap();
    let files = approved.resources.as_ref().unwrap();
    f.service
        .execute_removal_plan(
            &casklet_contracts::ExecuteRemovalPlan {
                version: CONTRACT_VERSION,
                id: files.id.clone(),
                plan_hash: files.plan_hash.clone(),
                confirm_permanent: false,
            },
            "test",
        )
        .unwrap();
    assert!(f.service.repository.get(&f.cask.id).unwrap().is_some());
    let restarted = CaskService {
        repository: SqliteCasks::open(f.root.join("db/casklet.db")).unwrap(),
        storage: LocalStorage::new(f.root.join("casks")),
    };
    let resumed = restarted
        .pending_whole_cask_cleanup(
            &casklet_contracts::GetCask {
                version: CONTRACT_VERSION,
                id: f.cask.id.clone(),
            },
            Some(&files.id),
            "test",
        )
        .unwrap()
        .unwrap();
    assert_eq!(resumed.resources.as_ref().unwrap().state, "COMPLETE");
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    restarted
                        .execute_category_cleanup(
                            &ExecuteCategoryCleanup {
                                plan: resumed.clone(),
                                confirm_permanent: false,
                            },
                            "test",
                        )
                        .unwrap()
                })
            })
            .collect();
        for handle in handles {
            assert!(handle.join().unwrap().cask_removed);
        }
    });
    assert!(restarted.repository.get(&f.cask.id).unwrap().is_none());
    assert_eq!(fs::read(&f.source).unwrap(), b"user data");
    assert!(
        restarted
            .execute_category_cleanup(
                &ExecuteCategoryCleanup {
                    plan: resumed,
                    confirm_permanent: false
                },
                "test"
            )
            .unwrap()
            .cask_removed
    );
}

#[cfg(feature = "reviewed-removal")]
#[test]
fn whole_cask_keeps_failed_and_preserved_resources_available() {
    use casklet_contracts::{CreateCategoryCleanup, ExecuteCategoryCleanup};
    for change_after_approval in [false, true] {
        let f = Fixture::new();
        f.adopt(true).unwrap();
        if !change_after_approval {
            fs::write(&f.source, b"changed bytes").unwrap();
        }
        let plan = f
            .service
            .create_category_cleanup(
                &CreateCategoryCleanup {
                    scope: CreateRemovalPlan {
                        version: CONTRACT_VERSION,
                        cask_id: f.cask.id.clone(),
                        resource_ids: vec![],
                        category_key: None,
                        permanent: false,
                    },
                    installation_id: None,
                },
                "test",
            )
            .unwrap();
        let approved = f.service.approve_category_cleanup(&plan, "test").unwrap();
        if change_after_approval {
            fs::write(&f.source, b"changed bytes").unwrap();
        }
        let result = f.service.execute_category_cleanup(
            &ExecuteCategoryCleanup {
                plan: approved,
                confirm_permanent: false,
            },
            "test",
        );
        if change_after_approval {
            let partial = result.unwrap();
            assert!(!partial.cask_removed);
            assert_eq!(partial.resources.unwrap().state, "PARTIAL");
        } else {
            assert_eq!(result.unwrap_err().code, "CASK_NOT_REMOVABLE");
        }
        assert!(f.service.repository.get(&f.cask.id).unwrap().is_some());
        assert_eq!(
            f.service
                .repository
                .list_resources(&f.cask.id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(fs::read(&f.source).unwrap(), b"changed bytes");
    }
}
