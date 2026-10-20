const SYSTEM_LIBRARIES: &[&str] = &[
    "shell32.lib",
    "ole32.lib",
    "shlwapi.lib",
    "advapi32.lib",
    "user32.lib",
    "gdi32.lib",
    "uuid.lib",
];

fn main() {
    build_explorer();
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=installer-hooks.nsh");
    tauri_build::build();
}

fn build_explorer() {
    println!("cargo:rerun-if-changed=explorer/extension.cpp");
    println!("cargo:rerun-if-changed=explorer/extension.def");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let source = std::fs::read("explorer/extension.cpp").expect("Explorer adapter source");
    // A versioned content name avoids replacing a DLL still loaded in Explorer.
    let hash = source.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    });
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/explorer-bundle");
    std::fs::create_dir_all(&root).expect("Explorer bundle directory");
    let output = root.join(format!(
        "casklet-explorer-{}-{hash:016x}.dll",
        env!("CARGO_PKG_VERSION")
    ));
    println!(
        "cargo:rustc-env=CASKLET_EXPLORER_LIBRARY={}",
        output
            .file_name()
            .expect("Explorer library name")
            .to_string_lossy()
    );
    let object =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"))
            .join("explorer.obj");
    // A new COM identity is required as well as a new DLL filename: Explorer
    // can retain a class factory from the previous in-process adapter.
    let class_hash = (hash & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
    let tail = class_hash
        .to_be_bytes()
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(",");
    let include = object.parent().expect("Explorer output directory");
    std::fs::write(
        include.join("explorer-class-id.h"),
        format!("#define CASKLET_EXPLORER_CLASS_ID {{0x960e5343,0xe03e,0x47cd,{{{tail}}}}}\n"),
    )
    .expect("Explorer COM identity header");
    let compiler = cc::Build::new()
        .cpp(true)
        .static_crt(true)
        .debug(false)
        .opt_level(2)
        .get_compiler();
    let status = compiler
        .to_command()
        .args([
            "/LD",
            "/EHsc",
            "/std:c++17",
            "/utf-8",
            "/W4",
            "/WX",
            "/O2",
            "/DUNICODE",
            "/D_UNICODE",
            "explorer/extension.cpp",
        ])
        .arg(format!("/I{}", include.display()))
        .arg(format!("/Fe{}", output.display()))
        .arg(format!("/Fo{}", object.display()))
        .args(["/link", "/DEF:explorer/extension.def"])
        .args(SYSTEM_LIBRARIES)
        .status()
        .expect("MSVC Explorer adapter build");
    assert!(status.success(), "Explorer adapter compilation failed");
    let tests = object.with_file_name("explorer-tests.exe");
    let status = compiler
        .to_command()
        .args([
            "/EHsc",
            "/std:c++17",
            "/utf-8",
            "/W4",
            "/WX",
            "/DUNICODE",
            "/D_UNICODE",
            "explorer/tests.cpp",
        ])
        .arg(format!("/I{}", include.display()))
        .arg(format!("/Fe{}", tests.display()))
        .arg(format!(
            "/Fo{}",
            object.with_file_name("explorer-tests.obj").display()
        ))
        .arg("/link")
        .args(SYSTEM_LIBRARIES)
        .status()
        .expect("MSVC Explorer contract tests build");
    assert!(
        status.success(),
        "Explorer contract tests compilation failed"
    );
    println!(
        "cargo:rustc-env=CASKLET_EXPLORER_TEST_EXE={}",
        tests.display()
    );
    println!("cargo:rerun-if-changed=explorer/tests.cpp");
    prune_generated(&root, &output);
}

fn prune_generated(root: &std::path::Path, output: &std::path::Path) {
    for entry in std::fs::read_dir(root).expect("Explorer artifacts") {
        let path = entry.expect("Explorer artifact").path();
        if path != output
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("casklet-explorer-"))
        {
            std::fs::remove_file(path).expect("Remove obsolete generated Explorer artifact");
        }
    }
}
