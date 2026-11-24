"""Read-only upstream VIctory monitor. Standard library only.

Source heuristics are leads, never proof of correctness. No title data, no
upstream code execution, no guessed total Xbox API count.
"""
import argparse
import collections
import datetime as dt
import html
import json
import pathlib
import re
import subprocess
import unittest

SCHEMA = 2
REPO = "https://github.com/Marvisak/VIctory"
STATES = ("missing", "stubbed", "implemented", "tested", "verified", "unknown")
GROUPS = {
    "implementation": [
        ("api", "Required static and dynamic Xbox APIs"),
        ("interfaces", "Runtime interfaces and GPU vtables"),
        ("shaders", "All required shader stages and instructions"),
        ("formats", "Texture, audio and streaming formats"),
        ("memory", "Memory, threading and synchronization"),
    ],
    "correctness": [
        ("api_semantics", "API behavior against reference"),
        ("shader_output", "Shader output against reference"),
        ("gpu_output", "GPU rendering and synchronization correctness"),
        ("texture_output", "Texture decoding correctness"),
        ("audio_output", "Audio output correctness"),
        ("storage", "Save integrity and recovery"),
    ],
    "gameplay": [
        ("boot", "Boot and first-run screens"),
        ("menus", "Menus, profile and save selection"),
        ("intro", "Intro video and audio"),
        ("load", "Load into the world"),
        ("render", "Correct open-world rendering"),
        ("movement", "Movement and interaction"),
        ("driving", "Driving and vehicle physics"),
        ("combat", "Combat"),
        ("missions", "Mission progression"),
        ("story", "Story completion"),
        ("save", "Save, quit and reload in gameplay"),
        ("series_s", "Series S mode reaches playable world"),
    ],
    "production": [
        ("amd", "Supported AMD GPU stability"),
        ("nvidia", "Supported NVIDIA GPU stability"),
        ("intel", "Supported Intel GPU stability"),
        ("fps", "Agreed frame-rate and frame-pacing target"),
        ("stability", "Extended gameplay without crash or TDR"),
        ("memory_budget", "Agreed memory/VRAM budget"),
        ("regressions", "Regression suite and release qualification"),
    ],
}
# These are a proposed readiness checklist, not upstream acceptance criteria.
TOKEN = re.compile(r'//[^\n]*|/\*[\s\S]*?\*/|r(#+)".*?"\1|"(?:\\.|[^"\\])*"', re.S)
EXPORT = re.compile(r'\bhle_export!\s*\(\s*(\w+)\s*\)|\bHleExport\s*\{\s*name:\s*"([^"]+)"\s*,\s*address:\s*([^,}]+)', re.S)

def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True, encoding="utf-8", errors="replace").strip()

def file_at(root, sha, path):
    return git(root, "show", f"{sha}:{path}")

def clean_comments(s):
    # Preserve strings for explicit export names; strip comments.
    return TOKEN.sub(lambda m: " " if m.group().startswith(("//", "/*")) else m.group(), s)

def code_only(s):
    return TOKEN.sub(" ", s)

def body_of(code, name):
    matches = list(re.finditer(r"\bfn\s+" + re.escape(name) + r"\s*(?:<[^{};]*>)?\s*\(", code))
    if len(matches) != 1:
        return None
    start = matches[0].end()
    first = re.search(r"[{;]", code[start:])
    if not first or first.group() != "{":
        return None
    start += first.start()
    depth = 0
    for i in range(start, len(code)):
        if code[i] == "{":
            depth += 1
        elif code[i] == "}":
            depth -= 1
            if depth == 0:
                return code[start+1:i].strip()
    return None

def candidate(body):
    if body is None:
        return "unclassified"
    if re.search(r"\b(?:todo|unimplemented)!\s*\(", body):
        return "explicit stub marker candidate"
    if re.fullmatch(r"(?:return\s+)?(?:0|1|false|true|S_OK|E_NOTIMPL|NOT_SUPPORTED|E_FAIL|(?:std::)?ptr::null(?:_mut)?\(\))\s*;?", body) or not body:
        return "constant/no-op candidate"
    return "body present; behavior unverified"

def inventory(root, sha):
    base = "crates/victory-hle/src/modules"
    try:
        registry = code_only(file_at(root, sha, base + "/mod.rs"))
    except subprocess.CalledProcessError:
        return {"available": False, "reason": "registry layout not found", "exports": []}
    modules = sorted(set(re.findall(r"\b(\w+)::module\s*\(\s*\)", registry)))
    if not modules:
        return {"available": False, "reason": "registry layout changed", "exports": []}
    exports, warnings = [], []
    for mod in modules:
        paths = [f"{base}/{mod}.rs", f"{base}/{mod}/mod.rs"]
        for path in paths:
            try:
                source = file_at(root, sha, path)
                break
            except subprocess.CalledProcessError:
                pass
        else:
            warnings.append(f"{mod}: source not found")
            continue
        cleaned = clean_comments(source)
        code = code_only(source)
        entries = {}
        for m in EXPORT.finditer(cleaned):
            name = m[1] or m[2]
            target = m[1] or m[3].strip()
            local = target if re.fullmatch(r"\w+", target) else re.match(r"(\w+)\s+as\b", target)
            if isinstance(local, re.Match):
                local = local[1]
            body = body_of(code, local) if local else None
            # Test mentions are weak leads; never a passing test.
            test_region = re.split(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", code, maxsplit=1)
            mentioned = len(test_region) == 2 and bool(re.search(r"\b" + re.escape(name) + r"\b", test_region[1]))
            entries[name] = {
                "id": f"{mod}:{name}", "module": mod, "name": name,
                "source": f"{REPO}/blob/{sha}/{path}",
                "candidate": candidate(body), "test_reference": mentioned,
                "state": "unknown",
            }
        if not entries:
            warnings.append(f"{mod}: no recognized export declarations")
        exports.extend(entries.values())
    return {"available": not warnings, "warnings": warnings, "exports": sorted(exports, key=lambda e: e["id"])}

def shader_counts(readme):
    # Accept the current upstream prose and a structured table if adopted.
    stages = {"mesh": r"mesh", "vertex_pixel": r"vertex\s*/\s*pixel", "compute": r"compute"}
    result = {}
    for key, pattern in stages.items():
        m = re.search(rf"\b{pattern}\s+([\d,]+)\s+of\s+([\d,]+)", readme, re.I)
        if not m:
            m = re.search(rf"^\|\s*{pattern}\s*\|\s*([\d,]+)\s*\|\s*([\d,]+)\s*\|", readme, re.I | re.M)
        if m:
            n, d = (int(x.replace(",", "")) for x in m.groups())
            if 0 <= n <= d and d:
                result[key] = {"translated": n, "known": d}
    return result

def status_date(readme):
    m = re.search(r"^## Status \((\d{4}-\d{2}-\d{2})\)", readme, re.M)
    return m[1] if m else None

def reported(readme):
    # Narrow recognition, deliberately fail closed if upstream prose changes.
    rules = {
        "boot": (r"^\| Boot,.*\|\s*works", "reported working"),
        "menus": (r"^\| Start screen, profile and saves\s*\|\s*works", "reported working"),
        "intro": (r"^\| Intro video with sound\s*\|\s*works", "reported working"),
        "load": (r"^\| Loading into the world.*\|\s*works", "reported working"),
        "render": (r"rendering fails|scene after it is still dark", "reported blocked"),
        "series_s": (r"^\| Series S mode\s*\| Runs through the menus to the loading screen", "reported partial"),
        "stability": (r"Entering the world on a real GPU can currently hang", "reported blocked"),
        "audio_output": (r"Reverb is silent", "reported partial"),
        "shader_output": (r"untranslated shaders drop draws", "reported blocked"),
        "gpu_output": (r"Most of the scene is still missing", "reported blocked"),
    }
    return {k: state for k, (pattern, state) in rules.items() if re.search(pattern, readme, re.I | re.M)}

def evidence_at(data, sha):
    if data.get("schema") != 1:
        raise ValueError("unsupported evidence schema")
    exact, stale = {}, 0
    for item in data.get("records", []):
        if not re.fullmatch(r"[0-9a-f]{40}", item["commit"]):
            raise ValueError("evidence needs a full upstream SHA")
        if item["state"] not in STATES or item["state"] == "unknown":
            raise ValueError("invalid evidence state")
        if not item.get("source", "").startswith("https://") or not item.get("method") or not item.get("environment"):
            raise ValueError("evidence requires source, method and environment")
        if item["commit"] != sha:
            stale += 1
            continue
        if item["id"] in exact:
            raise ValueError("duplicate evidence ID for revision")
        exact[item["id"]] = item
    return exact, stale

def collect(root, sha, evidence, full=True):
    readme = file_at(root, sha, "README.md")
    inv = inventory(root, sha)
    exact, stale = evidence_at(evidence, sha)
    known = {e["id"] for e in inv["exports"]} | {i for rows in GROUPS.values() for i, _ in rows}
    # Explicit missing exports may be supplied only by reviewed evidence.
    for key, item in exact.items():
        if key not in known and item["state"] == "missing" and ":" in key:
            module, name = key.split(":", 1)
            inv["exports"].append({"id": key, "module": module, "name": name, "source": item["source"], "candidate": "reviewed missing requirement", "test_reference": False, "state": "missing"})
        elif key not in known:
            raise ValueError(f"unknown evidence ID: {key}")
    for e in inv["exports"]:
        if e["id"] in exact:
            e["state"] = exact[e["id"]]["state"]
            e["evidence"] = exact[e["id"]]
    report = reported(readme)
    categories = {}
    for category, items in GROUPS.items():
        rows = [{"id": key, "label": label, "reported": report.get(key, "not reported"), "state": exact.get(key, {}).get("state", "unknown"), "evidence": exact.get(key)} for key, label in items]
        categories[category] = {"total": len(rows), "verified": sum(r["state"] == "verified" for r in rows), "items": rows}
    counts = collections.Counter(e["state"] for e in inv["exports"])
    candidates = collections.Counter(e["candidate"] for e in inv["exports"])
    registered = sum(e["candidate"] != "reviewed missing requirement" for e in inv["exports"])
    return {
        "schema": SCHEMA, "commit": sha, "commit_date": git(root, "show", "-s", "--format=%cI", sha),
        "readme_date": status_date(readme), "shader_translation_reported": shader_counts(readme),
        "inventory": {"complete_recognized_layout": inv["available"], "registered_source_declarations": registered, "states": dict(counts), "candidates": dict(candidates), "warnings": inv.get("warnings", [inv.get("reason", "")] if not inv["available"] else []), "exports": inv["exports"] if full else []},
        "categories": categories, "stale_evidence_records": stale,
    }

def render(report, history):
    esc = lambda x: html.escape(str(x), quote=True)
    def link(url, label):
        return f'<a href="{esc(url)}">{esc(label)}</a>' if url.startswith("https://") else esc(label)
    def bar(n, d):
        return f'<progress value="{n}" max="{d}"></progress> {n}/{d} ({n/d:.1%})' if d else "N/A"
    out = ['<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>VIctory upstream monitor</title><style>body{font:15px system-ui;max-width:1100px;margin:2rem auto;padding:0 1rem;background:#111827;color:#e5e7eb}a{color:#93c5fd}table{border-collapse:collapse;width:100%;margin:1rem 0}td,th{text-align:left;padding:.55rem;border-bottom:1px solid #374151;overflow-wrap:anywhere}progress{width:10rem;accent-color:#34d399}small{color:#cbd5e1}section{margin:2rem 0}</style><main><h1>VIctory upstream monitor</h1>']
    out.append(f'<p>Upstream revision {link(REPO + "/commit/" + report["commit"], report["commit"][:12])} · {esc(report["commit_date"])} · README status {esc(report["readme_date"] or "unknown")}</p>')
    out.append('<p>No overall playability percentage. Unknown is not verified; source declarations and reported milestones are not correctness tests.</p><p><a href="report.json">JSON</a> · <a href="history.json">History JSON</a></p>')
    out.append('<h2>Upstream CI</h2>')
    checks = report.get("upstream_checks", [])
    out.append("<ul>" + "".join(f'<li>{link(c["url"], c["name"])}: {esc(c["result"])}</li>' for c in checks) + "</ul>" if checks else "<p>No check-run evidence retrieved for this revision.</p>")
    out.append('<h2>Reported shader translation</h2><table><tr><th>Stage</th><th>Published coverage</th></tr>')
    for key in ("mesh", "vertex_pixel", "compute"):
        v = report["shader_translation_reported"].get(key)
        out.append(f'<tr><td>{esc(key)}</td><td>{bar(v["translated"], v["known"]) if v else "Not reported"}</td></tr>')
    inv = report["inventory"]
    out.append(f'</table><h2>HLE source inventory</h2><p>{inv["registered_source_declarations"]} recognized registered declarations. No complete Xbox API denominator. Parser layout recognized: {esc(inv["complete_recognized_layout"])}.</p>')
    out.append('<table><tr><th>Evidence state</th><th>Count</th></tr>')
    for state in STATES:
        out.append(f'<tr><td>{state}</td><td>{inv["states"].get(state, 0)}</td></tr>')
    out.append('</table><p>Stub candidates are heuristic flags, not confirmed missing or stubbed APIs.</p>')
    for warning in inv["warnings"]:
        out.append(f'<p>{esc(warning)}</p>')
    out.append('<details><summary>Per-function inventory and candidate flags</summary><table><tr><th>Function</th><th>Evidence state</th><th>Source flag</th><th>Test reference</th></tr>')
    for e in inv["exports"]:
        out.append(f'<tr><td>{link(e["source"], e["id"])}</td><td>{esc(e["state"])}</td><td>{esc(e["candidate"])}</td><td>{esc(e["test_reference"])}</td></tr>')
    out.append('</table></details>')
    for name, category in report["categories"].items():
        out.append(f'<section><h2>{esc(name.title())}</h2><p>Verified checklist: {bar(category["verified"], category["total"])}</p><table><tr><th>Requirement</th><th>Upstream report</th><th>Reviewed state</th><th>Evidence</th></tr>')
        for row in category["items"]:
            ev = row["evidence"]
            proof = link(ev["source"], ev["method"]) + " · " + esc(ev["environment"]) if ev else "None at this revision"
            out.append(f'<tr><td>{esc(row["label"])}</td><td>{esc(row["reported"])}</td><td>{esc(row["state"])}</td><td>{proof}</td></tr>')
        out.append('</table></section>')
    out.append(f'<p>Stale evidence records (not counted): {report["stale_evidence_records"]}</p><h2>Upstream commit history</h2><table><tr><th>Commit</th><th>Date</th><th>Registered declarations</th><th>Mesh</th><th>Vertex/pixel</th><th>Compute</th></tr>')
    for row in history:
        cells = []
        for key in ("mesh", "vertex_pixel", "compute"):
            v = row["shader_translation_reported"].get(key)
            cells.append(bar(v["translated"], v["known"]) if v else "N/A")
        out.append(f'<tr><td>{link(REPO + "/commit/" + row["commit"], row["commit"][:12])}</td><td>{esc(row["commit_date"][:10])}</td><td>{row["inventory"]["registered_source_declarations"] if row["inventory"]["complete_recognized_layout"] else "Partial / unavailable"}</td><td>{cells[0]}</td><td>{cells[1]}</td><td>{cells[2]}</td></tr>')
    out.append('</table><p>History reflects source and published README snapshots, not new gameplay tests. Checklist thresholds need agreement before release qualification.</p></main></html>')
    return "\n".join(out)

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--upstream", type=pathlib.Path, required=True)
    p.add_argument("--evidence", type=pathlib.Path, required=True)
    p.add_argument("--output", type=pathlib.Path, required=True)
    p.add_argument("--history", type=int, default=20)
    p.add_argument("--checks", type=pathlib.Path)
    a = p.parse_args()
    if not 1 <= a.history <= 100:
        p.error("history must be 1–100")
    evidence = json.loads(a.evidence.read_text(encoding="utf-8"))
    sha = git(a.upstream, "rev-parse", "HEAD")
    revisions = git(a.upstream, "rev-list", "--first-parent", f"--max-count={a.history}", sha).splitlines()
    report = collect(a.upstream, sha, evidence)
    report["observed_at"] = dt.datetime.now(dt.timezone.utc).isoformat()
    raw_checks = json.loads(a.checks.read_text(encoding="utf-8")) if a.checks else {}
    report["upstream_checks"] = [{"name": c["name"], "result": c.get("conclusion") or c["status"], "url": c.get("html_url") or ""} for c in raw_checks.get("check_runs", []) if c.get("head_sha") == sha]
    history = [collect(a.upstream, revision, evidence, False) for revision in revisions]
    a.output.mkdir(parents=True, exist_ok=False)
    (a.output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (a.output / "history.json").write_text(json.dumps(history, indent=2) + "\n", encoding="utf-8")
    (a.output / "index.html").write_text(render(report, history), encoding="utf-8")
    print(f"Upstream {sha}; {report['inventory']['registered_source_declarations']} recognized registered declarations")
    for key, value in report["categories"].items():
        print(f"{key}: {value['verified']}/{value['total']} verified checklist items")

class MonitorTests(unittest.TestCase):
    def test_shader_counts(self):
        self.assertEqual(shader_counts("World: mesh 8,191 of 8,506, vertex/pixel 10,617 of 12,346, compute 1,643 of 2,539")["mesh"], {"translated": 8191, "known": 8506})
        self.assertEqual(shader_counts("mesh 2 of 1"), {})
        self.assertEqual(shader_counts("nothing known"), {})
    def test_candidates_are_not_proof(self):
        self.assertEqual(candidate("0"), "constant/no-op candidate")
        self.assertEqual(candidate("todo!()"), "explicit stub marker candidate")
        self.assertEqual(candidate(None), "unclassified")
        self.assertEqual(candidate("do_work()"), "body present; behavior unverified")
    def test_source_forms(self):
        s = '/* hle_export!(Fake) */ hle_export!(Real), HleExport { name: "Other", address: other as usize }'
        self.assertEqual([m[1] or m[2] for m in EXPORT.finditer(clean_comments(s))], ["Real", "Other"])
        self.assertEqual(body_of("fn real() { if x { y() } }", "real"), "if x { y() }")
    def test_stale_evidence(self):
        record = {"id":"boot","commit":"a"*40,"state":"verified","source":"https://example.org","method":"reference","environment":"test"}
        self.assertEqual(evidence_at({"schema":1,"records":[record]}, "b"*40), ({}, 1))
        self.assertIn("boot", evidence_at({"schema":1,"records":[record]}, "a"*40)[0])
    def test_reported_is_not_verified(self):
        self.assertEqual(reported("| Boot, first-boot screens | works, 30 fps |"), {"boot":"reported working"})

if __name__ == "__main__":
    import sys
    if sys.argv[1:] == ["--self-test"]:
        unittest.main(argv=[sys.argv[0]])
    else:
        main()
