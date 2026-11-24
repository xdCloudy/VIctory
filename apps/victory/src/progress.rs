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
        let result = (|| -> Result<()> {
            for (name, content) in [
                ("progress.svg", self.svg()),
                ("index.html", self.html()),
                ("metrics.tsv", self.tsv()),
            ] {
                let path = output.join(name);
                victory_core::ensure_outside_repo(&path)?;
                std::fs::write(path, content)?;
            }
            Ok(())
        })();
        if result.is_err() {
            // Leave the directory for diagnosis, but never advertise it as complete.
            return result;
        }
        println!("Report: {}", output.display());
        Ok(())
    }

    fn svg(&self) -> String {
        let height = 82 + self.rows.len() * 58 + self.notes.len() * 22;
        let mut s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="{height}" viewBox="0 0 960 {height}" role="img" aria-labelledby="title"><title id="title">{}</title><rect width="960" height="{height}" rx="12" fill="#111827"/><g font-family="Arial,sans-serif" fill="#f1f5f9"><text x="24" y="34" font-size="22" font-weight="bold">{}</text><text x="24" y="58" font-size="13" fill="#a5b4fc">Bars with denominators show measured coverage; counts alone are not coverage.</text>"#, xml(self.title), xml(self.title));
        for (i, row) in self.rows.iter().enumerate() {
            let y = 86 + i * 58;
            let _ = write!(s, r#"<text x="24" y="{y}" font-size="14">{}</text><text x="936" y="{y}" font-size="14" text-anchor="end">{}</text>"#, xml(&row.label), xml(&row.value()));
            if let Some(total) = row.total.filter(|&total| total > 0) {
                let width = 912.0 * row.count as f64 / total as f64;
                let _ = write!(s, r#"<rect x="24" y="{}" width="912" height="9" rx="4" fill="#374151"/><rect x="24" y="{}" width="{width:.2}" height="9" rx="4" fill="#34d399"/>"#, y+7, y+7);
            }
            let _ = write!(s, r#"<text x="24" y="{}" font-size="12" fill="#cbd5e1">{}</text>"#, y+34, xml(&row.detail));
        }
        for (i, note) in self.notes.iter().enumerate() {
            let y = 86 + self.rows.len() * 58 + i * 22;
            let _ = write!(s, r#"<text x="24" y="{y}" font-size="12" fill="#cbd5e1">{}</text>"#, xml(note));
        }
        s.push_str("</g></svg>\n");
        s
    }

    fn html(&self) -> String {
        let mut s = format!(r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{}</title><style>body{{font:16px system-ui;max-width:1000px;margin:2rem auto;padding:0 1rem;color:#182230}}table{{border-collapse:collapse;width:100%}}td,th{{text-align:left;border-bottom:1px solid #bbb;padding:.6rem}}th[scope=row]{{overflow-wrap:anywhere}}img{{max-width:100%;height:auto}}progress{{width:100%;accent-color:#087f5b}}@media(prefers-color-scheme:dark){{body{{background:#111827;color:#eee}}a{{color:#a5b4fc}}}}</style><main><h1>{}</h1><p><a href="metrics.tsv">Download machine-readable metrics</a> · <a href="progress.svg">SVG</a></p><table><thead><tr><th>Metric</th><th>Result</th><th>Scope</th></tr></thead><tbody>"#, xml(self.title), xml(self.title));
        for row in &self.rows {
            let bar = match row.total {
                Some(total) if total > 0 => format!(r#"<progress max="{total}" value="{}" aria-label="{}"></progress>"#, row.count, xml(&row.label)),
                _ => String::new(),
            };
            let _ = write!(s, r#"<tr><th scope="row">{}</th><td>{}{bar}</td><td>{}</td></tr>"#, xml(&row.label), xml(&row.value()), xml(&row.detail));
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

/// Public-safe, game-independent inventory. A registry entry may be a stub;
/// no denominator for all Xbox APIs is available in the source tree.
pub(crate) fn source(output: &Path) -> Result<()> {
    let mut rows = Vec::new();
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
            "Registration does not establish implementation correctness; stubs may be present.".into(),
            "No global Xbox API denominator is known. Counts are not gameplay progress.".into(),
            "No title files were read. Windows and non-Windows inventories differ.".into(),
        ],
        rows,
    }.write(output)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rejects_impossible_counts_before_writing() {
        let r = Report { title: "Invalid", notes: vec![], rows: vec![Row::coverage("x", 3, 2, "")] };
        let path = std::env::temp_dir().join("victory-invalid-progress-test");
        assert!(r.write(&path).is_err());
    }
}
