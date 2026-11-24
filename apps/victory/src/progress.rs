//! Asset-free reports of source inventory and locally measured coverage.
//! Report destinations must be outside the source tree; no game data is published.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};

pub(crate) struct Row {
    label: String,
    count: usize,
    total: Option<usize>,
    detail: String,
}

impl Row {
    pub(crate) fn coverage(label: impl Into<String>, count: usize, total: usize, detail: impl Into<String>) -> Self {
        Self { label: label.into(), count, total: Some(total), detail: detail.into() }
    }

    pub(crate) fn inventory(label: impl Into<String>, count: usize, detail: impl Into<String>) -> Self {
        Self { label: label.into(), count, total: None, detail: detail.into() }
    }

    fn value(&self) -> String {
        match self.total {
            Some(0) => "N/A (0/0)".into(),
            Some(total) => format!("{:.1}% ({}/{total})", self.count as f64 * 100.0 / total as f64, self.count),
            None => self.count.to_string(),
        }
    }
}

pub(crate) struct Report {
    pub(crate) title: &'static str,
    pub(crate) notes: Vec<String>,
    pub(crate) rows: Vec<Row>,
}

impl Report {
    pub(crate) fn write(&self, output: &Path) -> Result<()> {
        victory_core::ensure_outside_repo(output)?;
        for row in &self.rows {
            anyhow::ensure!(row.total.is_none_or(|total| row.count <= total), "invalid coverage for {}", row.label);
        }
        // A new directory per measurement keeps earlier snapshots immutable.
        std::fs::create_dir(output).with_context(|| format!("create fresh report directory {}", output.display()))?;
        for (name, content) in [
            ("progress.svg", self.svg()),
            ("index.html", self.html()),
            ("metrics.tsv", self.tsv()),
            ("metadata.txt", format!("{}\n{}\n", self.title, self.notes.join("\n"))),
        ] {
            let path = output.join(name);
            victory_core::ensure_outside_repo(&path)?;
            std::fs::write(path, content)?;
        }
        // Written last; history ignores interrupted snapshots.
        let complete = output.join("complete.txt");
        victory_core::ensure_outside_repo(&complete)?;
        std::fs::write(complete, "schema=1\n")?;
        println!("Report: {}", output.display());
        Ok(())
    }

    fn svg(&self) -> String {
        let height = 82 + self.rows.len() * 58 + self.notes.len() * 22;
        let mut s = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="{height}" viewBox="0 0 960 {height}" role="img" aria-labelledby="title"><title id="title">{}</title><rect width="960" height="{height}" rx="12" fill="#111827"/><g font-family="Arial,sans-serif" fill="#f1f5f9"><text x="24" y="34" font-size="22" font-weight="bold">{}</text><text x="24" y="58" font-size="13" fill="#a5b4fc">Bars with denominators show measured coverage; counts alone are not coverage.</text>"##, xml(self.title), xml(self.title));
        for (i, row) in self.rows.iter().enumerate() {
            let y = 86 + i * 58;
            let _ = write!(s, r##"<text x="24" y="{y}" font-size="14">{}</text><text x="936" y="{y}" font-size="14" text-anchor="end">{}</text>"##, xml(&row.label), xml(&row.value()));
            if let Some(total) = row.total.filter(|&total| total > 0) {
                let width = 912.0 * row.count as f64 / total as f64;
                let _ = write!(s, r##"<rect x="24" y="{}" width="912" height="9" rx="4" fill="#374151"/><rect x="24" y="{}" width="{width:.2}" height="9" rx="4" fill="#34d399"/>"##, y+7, y+7);
            }
            let _ = write!(s, r##"<text x="24" y="{}" font-size="12" fill="#cbd5e1">{}</text>"##, y+34, xml(&row.detail));
        }
        for (i, note) in self.notes.iter().enumerate() {
            let y = 86 + self.rows.len() * 58 + i * 22;
            let _ = write!(s, r##"<text x="24" y="{y}" font-size="12" fill="#cbd5e1">{}</text>"##, xml(note));
        }
        s.push_str("</g></svg>\n");
        s
    }

    fn html(&self) -> String {
        let mut s = format!(r##"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{}</title><style>body{{font:16px system-ui;max-width:1000px;margin:2rem auto;padding:0 1rem;color:#182230}}table{{border-collapse:collapse;width:100%}}td,th{{text-align:left;border-bottom:1px solid #bbb;padding:.6rem}}th[scope=row]{{overflow-wrap:anywhere}}img{{max-width:100%;height:auto}}progress{{width:100%;accent-color:#087f5b}}@media(prefers-color-scheme:dark){{body{{background:#111827;color:#eee}}a{{color:#a5b4fc}}}}</style><main><h1>{}</h1><p><a href="metrics.tsv">Download machine-readable metrics</a> · <a href="progress.svg">SVG</a></p><table><thead><tr><th>Metric</th><th>Result</th><th>Scope</th></tr></thead><tbody>"##, xml(self.title), xml(self.title));
        for row in &self.rows {
            let bar = match row.total {
                Some(total) if total > 0 => format!(r##"<progress max="{total}" value="{}" aria-label="{}"></progress>"##, row.count, xml(&row.label)),
                _ => String::new(),
            };
            let _ = write!(s, r##"<tr><th scope="row">{}</th><td>{}{bar}</td><td>{}</td></tr>"##, xml(&row.label), xml(&row.value()), xml(&row.detail));
        }
        s.push_str("</tbody></table><h2>Methodology</h2><ul>");
        for note in &self.notes {
            let _ = write!(s, "<li>{}</li>", xml(note));
        }
        s.push_str("</ul></main></html>\n");
        s
    }

    fn tsv(&self) -> String {
        let mut s = String::from("schema\tmetric\tcount\ttotal\tdetail\n");
        for row in &self.rows {
            let total = row.total.map_or_else(String::new, |n| n.to_string());
            let _ = writeln!(s, "1\t{}\t{}\t{total}\t{}", cell(&row.label), row.count, cell(&row.detail));
        }
        s
    }
}

fn cell(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn xml(s: &str) -> String {
    cell(s).replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;").replace(['\u{fffe}', '\u{ffff}'], "\u{fffd}")
}


/// Compare an explicitly ordered series of immutable reports. Reports with
/// different report types are rejected. An operator must keep the same corpus,
/// host and flags; matching report types do not prove comparability.
pub(crate) fn history(inputs: &[std::path::PathBuf], output: &Path) -> Result<()> {
    anyhow::ensure!(inputs.len() >= 2, "history needs at least two reports");
    victory_core::ensure_outside_repo(output)?;
    let mut previous: Option<(String, Option<String>)> = None;
    let mut rows = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        victory_core::ensure_outside_repo(input)?;
        anyhow::ensure!(std::fs::read_to_string(input.join("complete.txt"))? == "schema=1\n", "incomplete or unsupported report");
        let metadata = std::fs::read_to_string(input.join("metadata.txt"))?;
        let kind = metadata.lines().next().unwrap_or_default().to_owned();
        let context = metadata.lines().find(|line| line.starts_with("Mode:") || line.starts_with("Validation requested:") || line.starts_with("Host OS:")).map(str::to_owned);
        let identity = (kind, context);
        if let Some(expected) = &previous {
            anyhow::ensure!(expected == &identity, "cannot combine different report types or measurement modes");
        } else {
            previous = Some(identity);
        }
        let data = std::fs::read_to_string(input.join("metrics.tsv"))?;
        let mut lines = data.lines();
        anyhow::ensure!(lines.next() == Some("schema\tmetric\tcount\ttotal\tdetail"), "unsupported metrics schema");
        for line in lines {
            let fields: Vec<_> = line.split('\t').collect();
            anyhow::ensure!(fields.len() == 5 && fields[0] == "1", "malformed metric");
            let count = fields[2].parse::<usize>()?;
            let label = format!("Run {} — {}", i + 1, fields[1]);
            let row = if fields[3].is_empty() {
                Row::inventory(label, count, fields[4])
            } else {
                Row::coverage(label, count, fields[3].parse()?, fields[4])
            };
            rows.push(row);
        }
    }
    Report {
        title: "VIctory progress history",
        notes: vec![
            "Runs appear in supplied order, oldest first; each retains its own denominator.".into(),
            "Only compare like-for-like corpora, host, flags and build configuration.".into(),
            "Run numbers are ordering labels, not timestamps. Input directories are not exposed.".into(),
        ],
        rows,
    }.write(output)
}


/// Read only the maintainer-authored aggregate counts already published in the
/// README. Never use a locally generated corpus report for a public dashboard.
fn published_shader_rows(readme: &str) -> Result<Vec<Row>> {
    let (_, rest) = readme.split_once("<!-- progress:begin -->").context("missing published coverage table")?;
    let (table, _) = rest.split_once("<!-- progress:end -->").context("unterminated published coverage table")?;
    let mut rows = Vec::new();
    for line in table.lines().filter(|line| line.starts_with('|')).skip(2) {
        let fields: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
        anyhow::ensure!(fields.len() == 3, "invalid published coverage row");
        let count = fields[1].parse::<usize>()?;
        let total = fields[2].parse::<usize>()?;
        anyhow::ensure!(count <= total && total > 0, "invalid published coverage counts");
        rows.push(Row::coverage(format!("Reported world {} shaders", fields[0]), count, total, "Maintainer-reported README snapshot; not measured in CI"));
    }
    anyhow::ensure!(!rows.is_empty(), "published coverage table is empty");
    Ok(rows)
}

/// Public-safe, game-independent inventory. A registry entry may be a stub;
/// no denominator for all Xbox APIs is available in the source tree.
pub(crate) fn source(output: &Path) -> Result<()> {
    let readme = include_str!("../../../README.md");
    let mut rows = published_shader_rows(readme)?;
    let status = readme.lines().find(|line| line.starts_with("## Status (")).unwrap_or("Status date not specified").trim_start_matches("## ");
    let mut total = 0;
    let mut modules: Vec<_> = victory_hle::registry().iter().collect();
    modules.sort_by_key(|m| m.name);
    for module in &modules {
        let count = module.exports.len();
        total += count;
        rows.push(Row::inventory(module.name, count, format!("{} DLL patterns; registered exports", module.dlls.len())));
    }
    rows.insert(0, Row::inventory("Registered HLE exports", total, format!("{} registered modules on {}", modules.len(), std::env::consts::OS)));
    Report {
        title: "VIctory source inventory",
        notes: vec![
            format!("Published shader counts: README {status}. No CI game scan."),
            "Registration does not establish implementation correctness; stubs may be present.".into(),
            "No global Xbox API denominator is known. Counts are not gameplay progress.".into(),
            format!("Host OS: {}; no title files read.", std::env::consts::OS),
            "Windows and non-Windows inventories differ.".into(),
        ],
        rows,
    }.write(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_table_uses_existing_readme_counts() {
        let rows = published_shader_rows(include_str!("../../../README.md")).unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| row.total.is_some_and(|total| total >= row.count)));
        assert!(published_shader_rows("no table").is_err());
    }

    #[test]
    fn zero_and_unknown_are_not_percentages() {
        assert_eq!(Row::coverage("empty", 0, 0, "").value(), "N/A (0/0)");
        assert_eq!(Row::inventory("count", 3, "").value(), "3");
        assert_eq!(Row::coverage("half", 2, 4, "").value(), "50.0% (2/4)");
    }

    #[test]
    fn reports_escape_untrusted_labels_and_keep_denominators() {
        let r = Report {
            title: "Test <&>",
            notes: vec!["note".into()],
            rows: vec![Row::coverage("a<&\"\t", 1, 2, "detail\nline"), Row::inventory("count", 3, "")],
        };
        assert!(r.svg().contains("a&lt;&amp;&quot; "));
        assert!(r.svg().contains("width=\"456.00\""));
        assert!(r.html().contains("max=\"2\" value=\"1\""));
        assert!(r.tsv().contains("1\ta<&\" \t1\t2\tdetail line\n"));
        assert!(r.tsv().contains("1\tcount\t3\t\t\n"));
    }

    #[test]
    fn snapshots_and_history_round_trip() {
        let root = std::env::temp_dir().join(format!(
            "victory-progress-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let a = root.join("a");
        let b = root.join("b");
        let result = root.join("history");
        let report = |count| Report {
            title: "Test",
            notes: vec!["Mode: same".into()],
            rows: vec![Row::coverage("test metric", count, 4, "fixture")],
        };
        report(1).write(&a).unwrap();
        report(2).write(&b).unwrap();
        assert!(report(3).write(&a).is_err());
        history(&[a.clone(), b.clone()], &result).unwrap();
        let data = std::fs::read_to_string(result.join("metrics.tsv")).unwrap();
        assert!(data.contains("Run 1 — test metric\t1\t4"));
        assert!(data.contains("Run 2 — test metric\t2\t4"));
        std::fs::remove_file(b.join("complete.txt")).unwrap();
        assert!(history(&[a, b], &root.join("incomplete")).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_impossible_counts_before_writing() {
        let r = Report { title: "Invalid", notes: vec![], rows: vec![Row::coverage("x", 3, 2, "")] };
        let path = std::env::temp_dir().join("victory-invalid-progress-test");
        assert!(r.write(&path).is_err());
    }
}
