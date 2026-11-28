//! Terminal-only presentation. DTOs and application policies remain untouched.
use crate::Output;
use casklet_contracts::{ApiError, CaskLibraryView, CaskView, ResourceView};
use clap::ValueEnum;
use serde_json::Value;
use std::fmt::Write;
use std::io::IsTerminal;
use std::sync::OnceLock;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Color {
    Auto,
    Always,
    Never,
}
static MACHINE: OnceLock<bool> = OnceLock::new();
static SETTINGS: OnceLock<(bool, Color)> = OnceLock::new();
pub fn configure(verbose: bool, color: Color, output: Output) {
    let _ = SETTINGS.set((verbose, color));
    let _ = MACHINE.set(matches!(output, Output::Json));
}
fn verbose() -> bool {
    SETTINGS.get().is_some_and(|s| s.0)
}
fn width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .or_else(|| {
            terminal_size::terminal_size().map(|(terminal_size::Width(w), _)| usize::from(w))
        })
        .unwrap_or(80)
        .clamp(10, 200)
}
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
fn label(s: &str) -> String {
    let friendly = match s {
        "categoryKey" => Some("Category"),
        "sizeBytes" => Some("Size"),
        "kind" => Some("Type"),
        "path" => Some("Location"),
        "createdAtMs" => Some("Created"),
        "updatedAtMs" => Some("Updated"),
        "expiresAtMs" => Some("Expires"),
        "sizeMeasurement" => Some("Measurement"),
        "managedLogicalBytes" => Some("Owned files (logical)"),
        "referencedLogicalBytes" => Some("Referenced files (logical)"),
        "ownedApplicationBytes" => Some("Owned applications"),
        "referenceApplicationBytes" => Some("Referenced applications"),
        "physicalBytes" => Some("Physical size"),
        "reclaimableMinBytes" => Some("Reclaimable minimum"),
        "reclaimableMaxBytes" => Some("Reclaimable maximum"),
        _ => None,
    };
    if let Some(friendly) = friendly {
        return friendly.into();
    }
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push(' ');
        }
        result.push(if c == '_' { ' ' } else { c });
    }
    let mut chars = result.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}
pub fn bytes(n: u64) -> String {
    let (divisor, unit) = if n >= 1 << 40 {
        (1_u64 << 40, "TiB")
    } else if n >= 1 << 30 {
        (1 << 30, "GiB")
    } else if n >= 1 << 20 {
        (1 << 20, "MiB")
    } else if n >= 1 << 10 {
        (1 << 10, "KiB")
    } else {
        return format!("{n} B");
    };
    // Integer arithmetic avoids losing precision for very large inventories.
    format!("{}.{:01} {unit}", n / divisor, (n % divisor) * 10 / divisor)
}
pub fn date(ms: i64) -> String {
    // Proleptic Gregorian civil date, UTC; no local-time or timezone dependency.
    let days = ms.div_euclid(86_400_000) + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month = (5 * doy + 2) / 153;
    let day = doy - (153 * month + 2) / 5 + 1;
    let m = month + if month < 10 { 3 } else { -9 };
    format!(
        "{:04}-{m:02}-{day:02} {:02}:{:02} UTC",
        year + i64::from(m <= 2),
        ms.div_euclid(3_600_000).rem_euclid(24),
        ms.div_euclid(60_000).rem_euclid(60)
    )
}
fn hidden(key: &str) -> bool {
    matches!(
        key,
        "version"
            | "revision"
            | "managedStorageRoot"
            | "sourceKey"
            | "management"
            | "containment"
            | "provenance"
            | "correlationId"
    ) || key == "id"
        || key.ends_with("Id")
}
fn scalar(key: &str, v: &Value) -> String {
    if (key.ends_with("Bytes") || key.ends_with("BytesMin") || key.ends_with("BytesMax"))
        && let Some(n) = v.as_u64()
    {
        return bytes(n);
    }
    if key.ends_with("AtMs")
        && let Some(n) = v.as_i64()
    {
        return date(n);
    }
    match v {
        Value::Null => "Unknown".into(),
        Value::Bool(b) => if *b { "Yes" } else { "No" }.into(),
        Value::String(s) => {
            if let Some(unc) = s.strip_prefix(r"\\?\UNC\") {
                return clean(&format!(r"\\{unc}"));
            }
            let display = s.strip_prefix(r"\\?\").unwrap_or(s);
            clean(display)
        }
        _ => v.to_string(),
    }
}
fn fields(v: &Value, out: &mut String, depth: usize) {
    match v {
        Value::Object(map) => {
            for (key, value) in map {
                if !verbose() && hidden(key) {
                    continue;
                }
                let title = label(key);
                if value.is_object() || value.is_array() {
                    let _ = writeln!(out, "{}{}", "  ".repeat(depth), title);
                    fields(value, out, depth + 1);
                } else {
                    let _ = writeln!(
                        out,
                        "{}{}: {}",
                        "  ".repeat(depth),
                        title,
                        scalar(key, value)
                    );
                }
            }
        }
        Value::Array(rows) => {
            if rows.is_empty() {
                out.push_str("No entries.\n");
            }
            for (index, row) in rows.iter().enumerate() {
                if index > 0 {
                    out.push('\n');
                }
                fields(row, out, depth);
            }
        }
        _ => {
            out.push_str(&scalar("", v));
            out.push('\n');
        }
    }
}
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return "No entries.\n".into();
    }
    let sizes: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rows.iter()
                .map(|r| UnicodeWidthStr::width(r[i].as_str()))
                .max()
                .unwrap_or(0)
                .max(h.len())
        })
        .collect();
    // Vertical cards retain every value on narrow terminals, including paths and IDs.
    if sizes.iter().sum::<usize>() + 2 * (headers.len() - 1) > width()
        || rows.iter().flatten().any(|s| !s.is_ascii())
    {
        return rows
            .iter()
            .map(|r| {
                headers
                    .iter()
                    .zip(r)
                    .fold(String::new(), |mut out, (h, v)| {
                        let _ = writeln!(out, "{h}: {}", clean(v));
                        out
                    })
                    + "\n"
            })
            .collect();
    }
    let render = |r: &[String]| {
        r.iter()
            .enumerate()
            .map(|(i, s)| format!("{:<width$}", clean(s), width = sizes[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_owned()
            + "\n"
    };
    let mut out = render(&headers.iter().map(|s| (*s).into()).collect::<Vec<_>>());
    out.push_str(&render(
        &sizes.iter().map(|n| "-".repeat(*n)).collect::<Vec<_>>(),
    ));
    for row in rows {
        out.push_str(&render(row));
    }
    out
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consumes temporary serialized DTOs at the presentation boundary"
)]
pub fn document(value: Value, output: Output) -> String {
    if matches!(output, Output::Json) {
        return format!("{value}\n");
    }
    if let Value::Array(rows) = &value {
        // Compact common inventory columns; full details remain available via get/verbose/JSON.
        if !verbose() && !rows.is_empty() && rows.iter().all(Value::is_object) {
            let candidates = [
                "name",
                "applicationName",
                "relativePath",
                "kind",
                "ownership",
                "shared",
                "categoryKey",
                "sizeBytes",
                "displayVersion",
                "state",
                "step",
            ];
            let keys: Vec<_> = candidates
                .into_iter()
                .filter(|k| rows.iter().any(|r| r.get(*k).is_some()))
                .collect();
            if !keys.is_empty() {
                let labels: Vec<_> = keys.iter().map(|k| label(k)).collect();
                let headers: Vec<_> = labels.iter().map(String::as_str).collect();
                return table(
                    &headers,
                    &rows
                        .iter()
                        .map(|r| keys.iter().map(|k| scalar(k, &r[*k])).collect())
                        .collect::<Vec<_>>(),
                );
            }
        }
    }
    let mut out = String::new();
    fields(&value, &mut out, 0);
    out
}
pub fn library(library: &CaskLibraryView) -> String {
    let rows = library
        .casks
        .iter()
        .map(|s| {
            vec![
                s.cask.name.clone(),
                s.cask.state.to_lowercase(),
                s.item_count.to_string(),
                bytes(s.managed_bytes.saturating_add(s.referenced_bytes)),
                s.unknown_entries.to_string(),
                date(s.cask.updated_at_ms)[..10].to_owned(),
            ]
        })
        .collect::<Vec<_>>();
    let mut out = format!(
        "Casks ({})\n\n{}",
        rows.len(),
        table(
            &[
                "Name",
                "Status",
                "Items",
                "Known size",
                "Unknown",
                "Updated"
            ],
            &rows
        )
    );
    if verbose() {
        for s in &library.casks {
            let _ = writeln!(out, "{}: {}", clean(&s.cask.name), s.cask.id);
        }
    }
    if rows.is_empty() {
        out.push_str("Next: casklet cask create \"My Work\"\n");
    }
    out
}
pub fn overview(cask: &CaskView, resources: &[ResourceView], library: &CaskLibraryView) -> String {
    let owned = resources.iter().filter(|r| r.ownership == "OWNED").count();
    let summary = library.casks.iter().find(|s| s.cask.id == cask.id);
    let mut out = format!(
        "{}\n\nStatus: {}\nDescription: {}\nCreated: {}\nUpdated: {}\n\nInventory\nFiles and folders: {}\nOwned files and folders: {owned}\nReferenced files and folders: {}\n",
        clean(&cask.name),
        cask.state.to_lowercase(),
        clean(&cask.description),
        date(cask.created_at_ms),
        date(cask.updated_at_ms),
        resources.len(),
        resources.len() - owned
    );
    if let Some(s) = summary {
        let _ = write!(
            out,
            "Applications: {}\nTotal items: {}\n\nStorage (saved logical sizes)\nOwned: {}\nReferenced: {}\nUnknown entries: {}\nShared content may also appear in other Casks.\n",
            s.application_count,
            s.item_count,
            bytes(s.managed_bytes),
            bytes(s.referenced_bytes),
            s.unknown_entries
        );
    }
    if verbose() {
        let _ = write!(out, "ID: {}\nRevision: {}\n", cask.id, cask.revision);
    }
    out
}
pub fn finish(text: &str, output: Output) -> String {
    if matches!(output, Output::Json) {
        return serde_json::from_str::<Value>(text).map_or_else(
            |_| format!("{}\n", serde_json::json!({"message":text.trim_end()})),
            |_| text.into(),
        );
    }
    let mut out = String::new();
    let available_width = width();
    for line in text.lines() {
        let line = clean(line);
        // Preserve all characters; wrapping does not truncate paths or review hashes.
        let mut count = 0;
        for c in line.chars() {
            let cells = UnicodeWidthChar::width(c).unwrap_or(0);
            if count + cells > available_width {
                out.push('\n');
                count = 0;
            }
            out.push(c);
            count += cells;
        }
        out.push('\n');
    }
    let color = SETTINGS.get().map_or(Color::Auto, |s| s.1);
    let enabled = matches!(color, Color::Always)
        || matches!(color, Color::Auto)
            && std::io::stdout().is_terminal()
            && std::env::var_os("NO_COLOR").is_none()
            && std::env::var("TERM").as_deref() != Ok("dumb");
    if enabled {
        out.lines()
            .enumerate()
            .map(|(i, line)| {
                if line.starts_with("Error:") || line.contains("[failed") {
                    format!("\x1b[31m{line}\x1b[0m\n")
                } else if line.contains("preserve") || line.contains("Unknown entries:") {
                    format!("\x1b[33m{line}\x1b[0m\n")
                } else if line.starts_with("Created")
                    || line.starts_with("Referenced:")
                    || line.starts_with("Wrote")
                {
                    format!("\x1b[32m{line}\x1b[0m\n")
                } else if i == 0 || line.ends_with(':') {
                    format!("\x1b[1;36m{line}\x1b[0m\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect()
    } else {
        out
    }
}
pub fn error(error: &ApiError, output: Output) {
    if matches!(output, Output::Json) {
        eprintln!("{}", serde_json::to_string(error).unwrap());
        return;
    }
    let message = if matches!(error.code.as_str(), "PERSISTENCE" | "INTERNAL" | "STORAGE")
        && !verbose()
    {
        "Casklet could not access its saved data. Check directory permissions and available disk space."
    } else {
        &error.message
    };
    eprintln!("Error: {}", clean(message));
    let hint = if error.code.contains("NOT_FOUND") {
        "Check the ID with --verbose cask list or --output json, then try again."
    } else if error.code.contains("CONSENT") {
        "Review ownership requirements in resource adopt --help before granting consent."
    } else if error.code.contains("PLAN") || error.code.contains("STALE") {
        "Create a fresh cleanup preview and review its actions before approval."
    } else {
        "Check the command's --help. Use --verbose for the error code and support reference."
    };
    eprintln!("Next: {hint}");
    if verbose() {
        eprintln!("Code: {}\nReference: {}", error.code, error.correlation_id);
    }
}
static LAST_PROGRESS: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
pub fn progress(message: &str) {
    if !std::io::stderr().is_terminal() || MACHINE.get().copied().unwrap_or(false) {
        return;
    }
    if let Ok(mut last) = LAST_PROGRESS.lock() {
        if last.is_some_and(|at| at.elapsed() < std::time::Duration::from_millis(500)) {
            return;
        }
        *last = Some(std::time::Instant::now());
    }
    if std::io::stderr().is_terminal() && !MACHINE.get().copied().unwrap_or(false) {
        eprintln!("Working: {}", clean(message));
    }
}
#[derive(Debug)]
pub struct Activity {
    stop: std::sync::mpsc::Sender<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Activity {
    pub fn start(output: Output) -> Option<Self> {
        if matches!(output, Output::Json) || !std::io::stderr().is_terminal() {
            return None;
        }
        let (stop, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            while rx.recv_timeout(std::time::Duration::from_secs(10)).is_err() {
                eprintln!("Working: waiting for Casklet operation...");
            }
        });
        Some(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes_and_dates() {
        assert_eq!(bytes(1024), "1.0 KiB");
        assert_eq!(bytes(0), "0 B");
        assert_eq!(date(0), "1970-01-01 00:00 UTC");
        assert_eq!(date(951_782_400_000), "2000-02-29 00:00 UTC");
    }
    #[test]
    fn machine_output_keeps_identifiers() {
        let v = serde_json::json!({"id":"private", "name":"Work"});
        let json = document(v.clone(), Output::Json);
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), v);
        let human = document(v, Output::Table);
        assert!(!human.contains("private"));
        assert!(human.contains("Name: Work"));
    }
    #[test]
    fn untrusted_controls_are_removed() {
        assert_eq!(clean("x\u{1b}[2J\ny"), "x [2J y");
    }
    #[test]
    fn empty_inventory_is_explicit() {
        assert_eq!(
            document(serde_json::json!([]), Output::Table),
            "No entries.\n"
        );
    }
}
