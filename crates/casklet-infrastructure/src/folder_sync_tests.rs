//! Real Windows folder reconciliation and cleanup fixtures, confined to target.
use super::*;
use casklet_application::{
    removal::RemovalRepository,
    resource::{ResourceRepository, ResourceStorage},
};
use casklet_contracts::*;

struct FolderFixture {
    root: PathBuf,
    folder: PathBuf,
    service: CaskService<SqliteCasks, LocalStorage>,
    cask: CaskView,
    resource: ResourceView,
}
impl FolderFixture {
    fn new(owned: bool) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("folder-sync-{}", uuid::Uuid::new_v4()));
        let folder = root.join("external/project");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("initial.txt"), b"initial").unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("db/casklet.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Folder sync".into(),
                },
                "test",
            )
            .unwrap();
        let resource = if owned {
            service
                .adopt_resource(
                    &AdoptResource {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id.clone(),
                        source_path: folder.to_string_lossy().into_owned(),
                        category_key: "other".into(),
                        confirm_ownership: true,
                    },
                    "test",
                    &|_, _| {},
                )
                .unwrap()
        } else {
            service
                .add_resource(
                    &AddResource {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id.clone(),
                        source_path: folder.to_string_lossy().into_owned(),
                        category_key: "other".into(),
                        mode: AddMode::Reference,
                    },
                    "test",
                    &|_, _| {},
                )
                .unwrap()
        };
        Self {
            root,
            folder,
            service,
            cask,
            resource,
        }
    }
    fn change(&self) {
        fs::create_dir_all(self.folder.join("nested/deep")).unwrap();
        fs::write(self.folder.join("nested/deep/later.txt"), b"later bytes").unwrap();
        fs::write(self.folder.join("initial.txt"), b"modified").unwrap();
    }
    fn contents(&self) -> ResourceContents {
        self.service
            .resource_contents(
                &GetResource {
                    version: CONTRACT_VERSION,
                    id: self.resource.id.clone(),
                },
                true,
                "test",
            )
            .unwrap()
    }
    fn plan(&self) -> RemovalPlanView {
        self.service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: self.cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                    permanent: false,
                },
                "test",
            )
            .unwrap()
    }
    #[cfg(feature = "reviewed-removal")]
    fn approve(&self, plan: &RemovalPlanView) {
        self.service
            .approve_removal_plan(
                &ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
    }
    #[cfg(feature = "reviewed-removal")]
    fn execute(&self, plan: &RemovalPlanView) -> RemovalPlanView {
        self.service
            .execute_removal_plan(
                &ExecuteRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                    confirm_permanent: false,
                },
                "test",
            )
            .unwrap()
    }
}
impl Drop for FolderFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn current_nested_contents_and_sizes_inherit_parent_without_child_grants() {
    for owned in [false, true] {
        let f = FolderFixture::new(owned);
        f.change();
        let contents = f.contents();
        assert_eq!(
            contents.ownership,
            if owned { "OWNED" } else { "REFERENCED" }
        );
        assert_eq!(contents.total_entries, 4);
        assert_eq!(contents.unknown_entries, 0);
        assert!(
            contents
                .entries
                .iter()
                .any(|entry| entry.relative_path.ends_with("later.txt"))
        );
        let summary = f
            .service
            .storage_summary(
                &GetStorageSummary {
                    version: CONTRACT_VERSION,
                    cask_id: f.cask.id.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(
            if owned {
                summary.managed_logical_bytes
            } else {
                summary.referenced_logical_bytes
            },
            19
        );
        let recorded = f
            .service
            .repository
            .get_resource(&f.resource.id)
            .unwrap()
            .unwrap();
        assert_eq!(recorded.size_bytes, Some(19));
        assert_eq!(
            f.service
                .repository
                .list_resources(&f.cask.id)
                .unwrap()
                .len(),
            1
        );
        fs::remove_file(f.folder.join("initial.txt")).unwrap();
        f.contents();
        assert_eq!(
            f.service
                .storage
                .measure_resource(
                    &f.resource.path,
                    casklet_domain::resource::ResourceKind::Directory,
                    true
                )
                .unwrap()
                .size_bytes,
            Some(11)
        );
        let restarted = CaskService {
            repository: SqliteCasks::open(f.root.join("db/casklet.db")).unwrap(),
            storage: LocalStorage::new(f.root.join("casks")),
        };
        assert_eq!(
            restarted
                .storage_summary(
                    &GetStorageSummary {
                        version: CONTRACT_VERSION,
                        cask_id: f.cask.id.clone()
                    },
                    "test"
                )
                .unwrap()
                .managed_logical_bytes,
            if owned { 11 } else { 0 }
        );
    }
}

#[test]
fn large_listing_is_bounded_but_size_and_nested_contents_are_complete() {
    let f = FolderFixture::new(false);
    for index in 0..250 {
        fs::write(f.folder.join(format!("{index}.txt")), b"1234").unwrap();
    }
    let contents = f.contents();
    assert_eq!(contents.total_entries, 251);
    assert_eq!(contents.entries.len(), 200);
    assert_eq!(contents.unknown_entries, 0);
    assert_eq!(
        f.service
            .storage
            .measure_resource(
                &f.resource.path,
                casklet_domain::resource::ResourceKind::Directory,
                true
            )
            .unwrap()
            .size_bytes,
        Some(1007)
    );
}

#[test]
fn replacement_root_never_inherits_authority_and_added_hardlinks_are_preserved() {
    let f = FolderFixture::new(true);
    fs::rename(&f.folder, f.root.join("original")).unwrap();
    fs::create_dir_all(&f.folder).unwrap();
    fs::write(f.folder.join("initial.txt"), b"initial").unwrap();
    assert_eq!(f.plan().items[0].action, "PRESERVE");
    let replacement = f.contents();
    assert_eq!(replacement.availability, "CHANGED");
    assert_eq!(replacement.ownership, "UNVERIFIED");
    let another = FolderFixture::new(true);
    fs::hard_link(
        another.folder.join("initial.txt"),
        another.root.join("needed-elsewhere.txt"),
    )
    .unwrap();
    assert_eq!(another.plan().items[0].action, "PRESERVE");
}

#[test]
fn changed_legacy_folder_requires_explicit_consent_and_renewal_keeps_resource_id() {
    let f = FolderFixture::new(true);
    let connection = f.service.repository.connection().unwrap();
    let identity: String = connection
        .query_row(
            "SELECT identity_key FROM resource_locations WHERE resource_id=?1",
            [&f.resource.id],
            |row| row.get(0),
        )
        .unwrap();
    let legacy = identity.split_once('|').unwrap().1;
    connection
        .execute(
            "UPDATE resource_locations SET identity_key=?2 WHERE resource_id=?1",
            params![f.resource.id, legacy],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE resource_ownership_grants SET identity_key=?2 WHERE resource_id=?1",
            params![f.resource.id, legacy],
        )
        .unwrap();
    assert_eq!(f.plan().items[0].action, "TRASH");
    f.change();
    assert_eq!(f.plan().items[0].action, "PRESERVE");
    let mut request = AdoptResource {
        version: CONTRACT_VERSION,
        cask_id: f.cask.id.clone(),
        source_path: f.resource.path.clone(),
        category_key: "other".into(),
        confirm_ownership: false,
    };
    assert!(
        f.service
            .adopt_resource(&request, "test", &|_, _| {})
            .is_err()
    );
    request.confirm_ownership = true;
    let renewed = f
        .service
        .adopt_resource(&request, "test", &|_, _| {})
        .unwrap();
    assert_eq!(renewed.id, f.resource.id);
    assert_eq!(f.plan().items[0].action, "TRASH");
}

#[cfg(feature = "reviewed-removal")]
#[test]
fn real_owned_cleanup_recycles_all_current_contents_and_reference_cleanup_only_detaches() {
    for owned in [true, false] {
        let f = FolderFixture::new(owned);
        f.change();
        let plan = f.plan();
        assert_eq!(plan.items[0].action, if owned { "TRASH" } else { "DETACH" });
        assert_eq!(plan.items[0].expected_size_bytes, Some(19));
        f.approve(&plan);
        let result = f.execute(&plan);
        assert_eq!(result.state, "COMPLETE");
        assert_eq!(result.items[0].state, "SUCCEEDED");
        assert!(
            f.service
                .repository
                .list_resources(&f.cask.id)
                .unwrap()
                .is_empty()
        );
        if owned {
            assert!(!f.folder.exists());
            let quarantine = f.folder.parent().unwrap().join(format!(
                ".casklet-removal-{}-{}",
                result.run_id.as_ref().unwrap(),
                result.items[0].id
            ));
            assert!(!quarantine.exists());
        } else {
            assert_eq!(
                fs::read(f.folder.join("nested/deep/later.txt")).unwrap(),
                b"later bytes"
            );
        }
        assert!(f.service.repository.get(&f.cask.id).unwrap().is_some());
    }
}

#[cfg(feature = "reviewed-removal")]
#[test]
fn change_after_approval_is_failed_not_completed_and_new_preview_covers_changes() {
    let f = FolderFixture::new(true);
    let plan = f.plan();
    f.approve(&plan);
    f.change();
    let result = f.execute(&plan);
    assert_eq!(result.state, "PARTIAL");
    assert_eq!(result.items[0].state, "FAILED");
    assert_eq!(
        result.items[0].result_code.as_deref(),
        Some("REVALIDATION_FAILED")
    );
    assert!(f.folder.join("nested/deep/later.txt").exists());
    assert_eq!(
        f.service
            .repository
            .list_resources(&f.cask.id)
            .unwrap()
            .len(),
        1
    );
    f.service
        .close_uncertain_removal(
            &CloseRemovalRun {
                version: CONTRACT_VERSION,
                run_id: result.run_id.unwrap(),
            },
            "test",
        )
        .unwrap();
    let fresh = f.plan();
    assert_ne!(fresh.id, plan.id);
    assert_eq!(fresh.items[0].expected_size_bytes, Some(19));
    f.approve(&fresh);
    assert_eq!(f.execute(&fresh).state, "COMPLETE");
    assert!(!f.folder.exists());
}

#[cfg(feature = "reviewed-removal")]
#[test]
fn locked_file_is_reported_incomplete_and_never_detached_or_deleted() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = FolderFixture::new(true);
    let plan = f.plan();
    f.approve(&plan);
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(f.folder.join("initial.txt"))
        .unwrap();
    let result = f.execute(&plan);
    assert_eq!(result.state, "PARTIAL");
    assert_ne!(result.items[0].state, "SUCCEEDED");
    assert!(f.folder.exists());
    assert_eq!(
        f.service
            .repository
            .list_resources(&f.cask.id)
            .unwrap()
            .len(),
        1
    );
    drop(lock);
    let journal = f.service.repository.get_plan(&result.id).unwrap().unwrap();
    assert_eq!(journal.state, "PARTIAL");
}
