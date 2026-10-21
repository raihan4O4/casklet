//! Fail-closed probes: metadata loading must work without any content adapter access.
use super::*;
use casklet_application::{library::StorageBrowser, resource::*};
use casklet_contracts::*;
use casklet_domain::resource::{IntakeMode, ResourceKind};

struct NoContentAccess;
impl StorageBrowser for NoContentAccess {
    fn storage_root(&self) -> String {
        "configured metadata root".into()
    }
    fn open_directory(&self, _: &str) -> Result<(), String> {
        panic!("directory opened during load")
    }
}
impl ManifestProjection for NoContentAccess {
    fn project(
        &self,
        _: &Cask,
        _: &[ResourceView],
        _: &[CategoryView],
        _: &[DetectedApplicationView],
    ) -> Result<(), String> {
        panic!("manifest written during load")
    }
}
impl ResourceStorage for NoContentAccess {
    fn application_sizes(
        &self,
        _: &[ApplicationStorageEntry],
    ) -> Result<Vec<ApplicationSize>, String> {
        panic!("application scan during load")
    }
    fn measure_resource(&self, _: &str, _: ResourceKind, _: bool) -> Option<ResourceMeasurement> {
        panic!("resource measurement during load")
    }
    fn inspect(&self, _: &str, _: &dyn Fn(u64, u64)) -> Result<Observation, String> {
        panic!("content inspection during load")
    }
    fn inspect_reference(&self, _: &str, _: &dyn Fn(u64, u64)) -> Result<Observation, String> {
        panic!("reference inspection during load")
    }
    fn availability(&self, _: &str, _: ResourceKind) -> &'static str {
        panic!("filesystem stat during load")
    }
    fn allocate(&self, _: &Cask, _: &str, _: &str) -> IntakePaths {
        panic!("allocation during load")
    }
    fn materialize(
        &self,
        _: &Observation,
        _: &IntakePaths,
        _: IntakeMode,
        _: &dyn Fn(u64, u64),
        _: &dyn Fn() -> bool,
    ) -> Result<Option<String>, String> {
        panic!("copy during load")
    }
    fn finish_staged(
        &self,
        _: &Cask,
        _: &casklet_domain::resource::Resource,
        _: &IntakePaths,
        _: &Observation,
    ) -> Result<Option<String>, String> {
        panic!("move during load")
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one cross-layer fixture proves persisted loading and stale evidence rejection with all content access forbidden"
)]
fn metadata_loading_never_touches_content_and_preserves_terabyte_snapshots_after_restart() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("casklet-metadata-{}", uuid::Uuid::new_v4()));
    let service = service_at(&root, root.join("casks")).unwrap();
    let cask = service
        .create(
            &CreateCask {
                version: CONTRACT_VERSION,
                name: "Large references".into(),
            },
            "test",
        )
        .unwrap();
    let folder = root.join("external");
    fs::create_dir_all(folder.join("nested")).unwrap();
    fs::write(folder.join("nested/content.txt"), b"no scan needed").unwrap();
    let reference = service
        .add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: cask.id.clone(),
                source_path: folder.to_string_lossy().into_owned(),
                mode: AddMode::Reference,
                category_key: "other".into(),
            },
            "test",
            &|_, _| {},
        )
        .unwrap();
    assert!(
        reference.size_bytes.is_none(),
        "reference registration must not enumerate descendants"
    );
    assert_eq!(reference.ownership, "REFERENCED");
    let connection = service.repository.connection().unwrap();
    let terabyte = 1_i64 << 40;
    connection
        .execute(
            "UPDATE resources SET size_bytes=?1,unknown_entries=0 WHERE id=?2",
            params![terabyte, reference.id],
        )
        .unwrap();
    connection.execute("UPDATE resource_locations SET path_display='Z:/offline-terabyte-reference' WHERE resource_id=?1", [&reference.id]).unwrap();
    connection.execute("INSERT INTO applications(id,canonical_name,created_at_ms) VALUES ('large-app','Large App',1)", []).unwrap();
    connection.execute("INSERT INTO windows_installations(id,application_id,source_key,scope,display_name,evidence_json,evidence_sha256,state,first_seen_at_ms,last_seen_at_ms,last_verified_at_ms) VALUES ('large-install','large-app','large-registration','USER','Large App','{}','evidence','PRESENT',1,1,1)", []).unwrap();
    connection.execute("INSERT INTO windows_cask_associations(id,cask_id,installation_id,created_at_ms) VALUES ('link',?1,'large-install',1)", [&cask.id]).unwrap();
    drop(connection);
    let entries = service
        .repository
        .application_storage_entries(&cask.id)
        .unwrap();
    service
        .repository
        .record_application_sizes(
            &entries,
            &[ApplicationSize::Known {
                bytes: 1 << 40,
                estimated: true,
            }],
        )
        .unwrap();
    let fast = CaskService {
        repository: SqliteCasks::open(root.join("db/casklet.db")).unwrap(),
        storage: NoContentAccess,
    };
    let start = std::time::Instant::now();
    let library = fast
        .cask_library(
            &ListCasks {
                version: CONTRACT_VERSION,
            },
            "test",
        )
        .unwrap();
    let resources = fast
        .list_resource_metadata(
            &ListResources {
                version: CONTRACT_VERSION,
                cask_id: cask.id.clone(),
            },
            "test",
        )
        .unwrap();
    let summary = fast
        .storage_summary_metadata(
            &GetStorageSummary {
                version: CONTRACT_VERSION,
                cask_id: cask.id.clone(),
            },
            "test",
        )
        .unwrap();
    eprintln!(
        "Offline 1 TiB folder + 1 TiB application metadata load: {:?}",
        start.elapsed()
    );
    assert_eq!(resources[0].size_bytes, Some(1 << 40));
    assert_eq!(summary.referenced_logical_bytes, 1 << 40);
    assert_eq!(summary.reference_application_bytes, 1 << 40);
    assert_eq!(library.casks[0].referenced_bytes, 2 << 40);
    assert_eq!(library.casks[0].item_count, 2);
    // Changed registration evidence invalidates old presentation values; stale workers cannot restore them.
    fast.repository
        .connection()
        .unwrap()
        .execute(
            "UPDATE windows_installations SET evidence_sha256='new-evidence'",
            [],
        )
        .unwrap();
    fast.repository
        .record_application_sizes(
            &entries,
            &[ApplicationSize::Known {
                bytes: 42,
                estimated: false,
            }],
        )
        .unwrap();
    assert!(matches!(
        fast.repository
            .cached_application_sizes(
                &fast
                    .repository
                    .application_storage_entries(&cask.id)
                    .unwrap()
            )
            .unwrap()[0],
        ApplicationSize::Unknown
    ));
}
