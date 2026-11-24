//! Which imports do titles need, and which do we cover?

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use victory_hle::{HleResolver, HostPolicy};
use victory_loader::Provenance;
use victory_pe::{ImageInfo, ImportSymbol};

use crate::progress::{Report, Row};

#[derive(Default)]
struct DllStats {
    functions: BTreeSet<ImportSymbol>,
    hle: usize,
    host: usize,
    missing: usize,
    images: BTreeSet<PathBuf>,
}

pub fn run(paths: Vec<PathBuf>, strict: bool, list_missing: bool, progress_dir: Option<&Path>) -> Result<()> {
    if let Some(output) = progress_dir {
        victory_core::ensure_outside_repo(output)?;
        anyhow::ensure!(strict || cfg!(windows), "host coverage needs Windows (or use --strict)");
    }
    let paths = crate::default_paths(paths)?;
    let resolver = HleResolver::new(if strict {
        HostPolicy::none()
    } else {
        HostPolicy::default()
    });

    let mut images = 0usize;
    let mut failed = Vec::new();
    let mut walk_errors = 0usize;
    let mut header_errors = 0usize;
    let mut dlls: BTreeMap<String, DllStats> = BTreeMap::new();
    let mut missing: BTreeMap<(String, String), BTreeSet<PathBuf>> = BTreeMap::new();

    for root in &paths {
        for entry in walkdir::WalkDir::new(root) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => { walk_errors += 1; continue; }
            };
            let path = entry.path();
            if !entry.file_type().is_file() { continue; }
            match looks_like_pe(path) {
                Ok(true) => {},
                Ok(false) => continue,
                Err(_) => { header_errors += 1; continue; }
            }
            let info = match std::fs::read(path)
                .map_err(anyhow::Error::from)
                .and_then(|d| ImageInfo::parse(&d).map_err(anyhow::Error::from))
            {
                Ok(info) => info,
                Err(e) => {
                    failed.push((path.to_path_buf(), e.to_string()));
                    continue;
                }
            };
            images += 1;
            for module in info.imports.iter().chain(&info.delay_imports) {
                let key = module.dll.to_ascii_lowercase();
                let stats = dlls.entry(key.clone()).or_default();
                stats.images.insert(path.to_path_buf());
                for e in &module.entries {
                    let class = resolver.classify(&module.dll, &e.symbol);
                    if class.is_none() {
                        missing
                            .entry((key.clone(), e.symbol.to_string()))
                            .or_default()
                            .insert(path.to_path_buf());
                    }
                    // Count each dll!function once across all images.
                    // A name import's hint is not part of its identity.
                    let symbol = match &e.symbol {
                        ImportSymbol::Name { name, .. } => ImportSymbol::Name { hint: 0, name: name.clone() },
                        ImportSymbol::Ordinal(n) => ImportSymbol::Ordinal(*n),
                    };
                    if stats.functions.insert(symbol) {
                        match class {
                            Some(Provenance::Hle) => stats.hle += 1,
                            Some(Provenance::Host) => stats.host += 1,
                            Some(Provenance::Title) => unreachable!("census maps no title DLLs"),
                            None => stats.missing += 1,
                        }
                    }
                }
            }
        }
    }

    println!("{images} images scanned, {} unreadable", failed.len());
    for (p, e) in &failed {
        println!("  skipped {}: {e}", p.display());
    }
    println!(
        "\n{:<56} {:>6} {:>6} {:>6} {:>8} {:>7}",
        "dll", "funcs", "hle", "host", "missing", "images"
    );
    let (mut f, mut h, mut o, mut m) = (0, 0, 0, 0);
    for (dll, s) in &dlls {
        println!(
            "{dll:<56} {:>6} {:>6} {:>6} {:>8} {:>7}",
            s.functions.len(),
            s.hle,
            s.host,
            s.missing,
            s.images.len()
        );
        f += s.functions.len();
        h += s.hle;
        o += s.host;
        m += s.missing;
    }
    println!("{:<56} {f:>6} {h:>6} {o:>6} {m:>8}", "TOTAL");

    if let Some(output) = progress_dir {
        let mut rows = vec![Row::coverage("Total static import resolution", h + o, f, format!("HLE {h}; host {o}; unresolved {m}"))];
        for (dll, s) in &dlls {
            rows.push(Row::coverage(dll, s.hle + s.host, s.functions.len(), format!("HLE {}; host {}; unresolved {}; images {}", s.hle, s.host, s.missing, s.images.len())));
        }
        Report {
            title: "VIctory local import coverage",
            notes: vec![
                format!("Mode: {}; host OS: {}", if strict { "strict (HLE only)" } else { "HLE + host" }, std::env::consts::OS),
                format!("Scanned {images} PE images; {} parse failures; {walk_errors} walk errors; {header_errors} header read errors.", failed.len()),
                "Only distinct static and delay imports in successfully parsed images are counted.".into(),
                "Unresolved may include title DLL exports. Dynamic imports and interface methods are not measured.".into(),
                "Registration or host resolution does not establish semantic correctness. Keep reports outside the repository.".into(),
            ],
            rows,
        }.write(output)?;
    }

    if list_missing {
        let mut rows: Vec<_> = missing.into_iter().collect();
        rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
        println!("\nmissing (by number of images using it)");
        for ((dll, sym), users) in rows {
            println!("  {:>4}  {dll}!{sym}", users.len());
        }
    }
    Ok(())
}

/// Cheap pre-filter: an `MZ` header, regardless of extension (console
/// packages do not always use .exe/.dll).
fn looks_like_pe(path: &Path) -> std::io::Result<bool> {
    let mut magic = [0u8; 2];
    match std::fs::File::open(path)?.read_exact(&mut magic) {
        Ok(()) => Ok(&magic == b"MZ"),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
}
