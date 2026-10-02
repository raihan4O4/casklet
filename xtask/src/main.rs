use std::{
    env,
    path::Path,
    process::{Command, ExitCode},
};

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    println!("> {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("could not start {program}: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{program} exited with {status}"))
}

fn run_pnpm(args: &[&str]) -> Result<(), String> {
    let mut corepack_args = vec!["pnpm"];
    corepack_args.extend_from_slice(args);
    #[cfg(windows)]
    let corepack = "corepack.cmd";
    #[cfg(not(windows))]
    let corepack = "corepack";
    run(corepack, &corepack_args)
}

fn bootstrap_check() -> Result<(), String> {
    for path in [
        "Cargo.toml",
        "pnpm-lock.yaml",
        ".node-version",
        "rust-toolchain.toml",
    ] {
        if !Path::new(path).is_file() {
            return Err(format!("missing required file: {path}"));
        }
    }
    run("node", &["--version"])?;
    run_pnpm(&["--version"])
}

fn generate_contracts() -> Result<(), String> {
    let target = Path::new("apps/desktop/src/contracts/generated");
    if !target.is_dir() {
        return Err("generated-contract directory is missing".into());
    }
    std::fs::write(target.join("casklet.ts"), casklet_contracts::TYPESCRIPT)
        .map_err(|error| error.to_string())
}

fn check_contracts() -> Result<(), String> {
    let generated = std::fs::read_to_string("apps/desktop/src/contracts/generated/casklet.ts")
        .map_err(|error| error.to_string())?;
    if generated == casklet_contracts::TYPESCRIPT {
        Ok(())
    } else {
        Err("generated contracts differ; run cargo xtask generate-contracts".into())
    }
}

fn check() -> Result<(), String> {
    run("cargo", &["fmt", "--all", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["test", "--workspace", "--all-features"])?;
    // Feature profiles are separate invocations so Cargo feature unification cannot hide a gate bug.
    run(
        "cargo",
        &[
            "test",
            "-p",
            "casklet-cli",
            "-p",
            "casklet-desktop",
            "--locked",
        ],
    )?;
    run(
        "cargo",
        &[
            "test",
            "-p",
            "casklet-cli",
            "-p",
            "casklet-desktop",
            "--features",
            "alpha-cleanup",
            "--locked",
        ],
    )?;
    // Windows cannot replace the running xtask.exe during its own quality gate.
    run(
        "cargo",
        &[
            "build",
            "--workspace",
            "--exclude",
            "xtask",
            "--all-features",
        ],
    )?;
    run_pnpm(&["format:check"])?;
    run_pnpm(&["lint"])?;
    run_pnpm(&["check"])?;
    run_pnpm(&["test"])?;
    run_pnpm(&["build"])?;
    check_contracts()
}

#[cfg(windows)]
fn package(alpha: bool, allow_dirty: bool) -> Result<(), String> {
    let syft =
        env::var("SYFT_PATH").map_err(|_| "set SYFT_PATH to a verified syft.exe".to_string())?;
    let mut args = vec![
        "-NoProfile",
        "-File",
        "scripts/package-release.ps1",
        "-SyftPath",
        &syft,
    ];
    if alpha {
        args.extend(["-Alpha", "-OutputDirectory", "release/alpha"]);
    }
    if allow_dirty {
        args.push("-AllowDirty");
    }
    run("pwsh", &args)?;
    run(
        "pwsh",
        &[
            "-NoProfile",
            "-File",
            "scripts/verify-release.ps1",
            "-OutputDirectory",
            if alpha { "release/alpha" } else { "release" },
        ],
    )
}

#[cfg(not(windows))]
fn package(_alpha: bool, _allow_dirty: bool) -> Result<(), String> {
    Err("Release packaging requires Windows.".into())
}

#[cfg(windows)]
fn verify_release() -> Result<(), String> {
    run(
        "pwsh",
        &["-NoProfile", "-File", "scripts/verify-release.ps1"],
    )
}

#[cfg(not(windows))]
fn verify_release() -> Result<(), String> {
    Err("Release verification requires Windows.".into())
}

fn main() -> ExitCode {
    let result = match env::args().nth(1).as_deref() {
        Some("bootstrap-check") => bootstrap_check(),
        Some("generate-contracts") => generate_contracts(),
        Some("check" | "test") => check(),
        Some("test-integration") => run("cargo", &["test", "-p", "casklet-infrastructure", "-p", "casklet-cli", "--all-features", "--locked"]),
        Some("package") => package(false, false),
        Some("package-alpha") => package(true, env::args().any(|arg| arg == "--allow-dirty")),
        Some("verify-release") => verify_release(),
        _ => Err("usage: cargo xtask <bootstrap-check|generate-contracts|check|test|test-integration|package|package-alpha [--allow-dirty]|verify-release>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
