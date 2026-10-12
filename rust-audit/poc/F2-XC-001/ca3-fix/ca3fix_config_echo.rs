//! CA3-FIX (2026-10-09): F2-XC-001 echo fuzz through the real service config.
//!
//! Child module of `crate::config` in both the `validator` and the `sentinel`
//! binary (wired with `#[cfg(test)] #[path = ...] mod ca3fix_config_echo;`).
//! Every document goes through the production loader (`Config::load`, a file
//! on disk, `safenet_core::config::load`), and both the `Debug` rendering
//! (what `main` prints through `Termination`) and the `Display` rendering of
//! the error are searched for any 16-hex-digit window of a marker private key.
//!
//! Classes:
//! - `mutation`: the run-2 defect class. Every single-character deletion,
//!   insertion and replacement (17-character alphabet) at every position of
//!   the shipped sample with the marker as signer. Any echo here means the fix
//!   is incomplete.
//! - `moved`: the signer line moved to the end of the file and to just after
//!   every table header (an operator pasting it in the wrong place).
//! - `wrong_field`: the marker key pasted as the value of every other scalar
//!   field. A misplaced key may be echoed as the offending value; that is a
//!   different, narrower leak than the document echo and is only reported.

use super::Config;
use std::{collections::BTreeMap, path::PathBuf};

const MARKER: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ALPHABET: [char; 17] = [
    '"', '\'', '=', '\n', '[', ']', '#', ' ', '0', 'x', ',', '{', '}', '\\', '$', 'g', '.',
];

fn leaks(text: &str) -> bool {
    (0..=MARKER.len() - 16).any(|w| text.contains(&MARKER[w..w + 16]))
}

fn base_document() -> String {
    let sample = format!(
        "{}/{}.sample.toml",
        env!("CARGO_MANIFEST_DIR"),
        env!("CARGO_PKG_NAME")
    );
    let text = std::fs::read_to_string(&sample).unwrap();
    let mut out = String::new();
    let mut saw_signer = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("signer") && trimmed.contains('=') {
            out.push_str(&format!("signer = \"0x{MARKER}\"\n"));
            saw_signer = true;
        } else if trimmed.starts_with("rpc") && trimmed.contains('=') {
            out.push_str("rpc = \"http://127.0.0.1:9\"\n");
        } else if trimmed.starts_with("database") && trimmed.contains('=') {
            out.push_str("database = \"sqlite::memory:\"\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    assert!(saw_signer, "sample has no signer line");
    out
}

struct Run {
    path: PathBuf,
    documents: u64,
    errors: u64,
    leaked: BTreeMap<String, String>,
}

impl Run {
    async fn check(&mut self, label: String, document: &str) {
        tokio::fs::write(&self.path, document).await.unwrap();
        self.documents += 1;
        if let Err(err) = Config::load(&self.path).await {
            self.errors += 1;
            let rendered = format!("{err:?}\n{err}");
            if leaks(&rendered) {
                self.leaked.entry(label).or_insert(rendered);
            }
        }
    }
}

#[tokio::test]
async fn ca3fix_f2_xc_001_config_errors_never_echo_the_key() {
    let dir = std::env::var("CA3FIX_DB_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let path = PathBuf::from(format!("{dir}/ca3fix-config-{}-{}.toml", env!("CARGO_PKG_NAME"), std::process::id()));
    let base = base_document();
    let crate_name = env!("CARGO_PKG_NAME");

    // Control: the base document parses.
    tokio::fs::write(&path, &base).await.unwrap();
    Config::load(&path).await.expect("control document must parse");

    // Class 1: single-character mutations.
    let mut mutation = Run { path: path.clone(), documents: 0, errors: 0, leaked: BTreeMap::new() };
    let chars: Vec<char> = base.chars().collect();
    for i in 0..=chars.len() {
        if i < chars.len() {
            let mut d = chars.clone();
            d.remove(i);
            mutation.check(format!("delete@{i}"), &d.iter().collect::<String>()).await;
        }
        for c in ALPHABET {
            let mut d = chars.clone();
            d.insert(i, c);
            mutation.check(format!("insert@{i}:{c:?}"), &d.iter().collect::<String>()).await;
            if i < chars.len() && chars[i] != c {
                let mut d = chars.clone();
                d[i] = c;
                mutation.check(format!("replace@{i}:{c:?}"), &d.iter().collect::<String>()).await;
            }
        }
    }

    // Class 2: the signer line moved.
    let mut moved = Run { path: path.clone(), documents: 0, errors: 0, leaked: BTreeMap::new() };
    let signer_line = format!("signer = \"0x{MARKER}\"");
    let without: Vec<&str> = base.lines().filter(|l| l.trim() != signer_line).collect();
    moved.check("end-of-file".into(), &format!("{}\n{signer_line}\n", without.join("\n"))).await;
    for (i, line) in without.iter().enumerate() {
        if line.trim_start().starts_with('[') {
            let mut d: Vec<String> = without.iter().map(|s| s.to_string()).collect();
            d.insert(i + 1, signer_line.clone());
            moved.check(format!("after {}", line.trim()), &(d.join("\n") + "\n")).await;
        }
    }

    // Class 3: the key pasted as the value of every other scalar field.
    let mut wrong_field = Run { path: path.clone(), documents: 0, errors: 0, leaked: BTreeMap::new() };
    for (i, line) in base.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with('#') || t.starts_with('[') || !t.contains('=') || t.starts_with("signer") {
            continue;
        }
        let key = t.split('=').next().unwrap().trim().to_string();
        for value in [format!("\"0x{MARKER}\""), format!("\"{MARKER}\"")] {
            let d: Vec<String> = base
                .lines()
                .enumerate()
                .map(|(j, l)| if j == i { format!("{key} = {value}") } else { l.to_string() })
                .collect();
            wrong_field.check(format!("{key} := {}", &value[..4]), &(d.join("\n") + "\n")).await;
        }
    }

    for (name, run) in [("mutation", &mutation), ("moved", &moved), ("wrong_field", &wrong_field)] {
        println!(
            "[CA3-FIX F2-XC-001 {crate_name}] class {name}: {} documents, {} rejected, {} echo the key",
            run.documents,
            run.errors,
            run.leaked.len()
        );
        for (label, rendered) in run.leaked.iter().take(12) {
            let short: String = rendered.lines().next().unwrap_or("").chars().take(220).collect();
            println!("    {label}: {short}");
        }
    }
    let _ = std::fs::remove_file(&path);
    assert!(mutation.leaked.is_empty(), "the run-2 defect class must never echo the key");
    assert!(moved.leaked.is_empty(), "a misplaced signer line must never echo the key");
}
