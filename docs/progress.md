# Upstream progress monitor (fork branch)

This monitor lives on `feat/progress-tracker` only. It checks out the current
`Marvisak/VIctory:main` separately each time; it does not sync, merge, publish
to Pages or change either repository's main branch. The existing local Rust
coverage commands are independent of this workflow.

## Running

1. Enable Actions in your fork if disabled.
2. Open the [Upstream progress monitor](https://github.com/xdCloudy/VIctory/actions/workflows/progress.yml).
3. Push to the tracker branch to trigger a run, or rerun an existing successful
   workflow run from its Actions page. Each rerun fetches current upstream main.
4. Download the `upstream-progress-<run>-<attempt>` artifact and open
   `index.html`; `report.json` and `history.json` contain the raw measurements.

GitHub normally requires a workflow_dispatch definition on the default branch
before showing a Run workflow button or accepting a manual dispatch. Because
this tracker must remain branch-only, use a branch push or Actions rerun instead.
For a fresh run without editing the tracker:

```sh
git switch feat/progress-tracker
git commit --allow-empty -m "chore: refresh upstream progress"
git push origin feat/progress-tracker
```

Scheduled GitHub workflows likewise run only from the default branch; no
schedule is installed. Artifacts are retained for 90 days and the report
reconstructs the latest 20 first-parent upstream commits on each run.

## What is measured

- **Implementation:** recognized HLE export declarations, per-module functions,
  probable constant/no-op or explicit stub candidates, test references and
  separately reviewed lifecycle states (missing, stubbed, implemented, tested,
  verified, unknown). No full Xbox API denominator is assumed. Source parsing
  is heuristic and may miss macro-generated or changed registry layouts.
- **Correctness:** API semantics, shader output, rendering, synchronization,
  texture/audio decoding and storage. Upstream CI check runs are recorded
  separately when accessible; a green suite does not verify every function.
- **Gameplay:** boot, menus, intro, load, rendering, movement, driving, combat,
  missions, story completion, saves and Series S mode. Narrow README matches
  show reported milestones, never automatic verification.
- **Production readiness:** AMD/NVIDIA/Intel stability, frame pacing,
  extended stability, memory and regression qualification. The checklist is
  proposed, not official upstream release criteria. Targets must be agreed
  before evidence can establish a pass.

Published shader counts come from the upstream README, with its status date,
and are not remeasured by the workflow. The inventory and commit history are
read directly from upstream Git. Unknown or stale evidence is never credited.

## Reviewed evidence

Edit `.github/scripts/progress-evidence.json` on this branch to record reviewed
results. Start with an empty list; do not mark a feature verified solely because
a README says it works. Each record must name the exact upstream commit, item
ID (from report.json), lifecycle state, HTTPS evidence URL, test method and
environment. Example (placeholder only, not actual evidence):

```json
{
  "schema": 1,
  "records": [
    {
      "id": "boot",
      "commit": "0000000000000000000000000000000000000000",
      "state": "tested",
      "source": "https://example.org/test-report",
      "method": "Boot smoke test",
      "environment": "Windows; GPU and driver version"
    }
  ]
}
```

Use `verified` only when the agreed behavior or threshold has been checked
against reference evidence. `tested` means a test was run and reviewed, not
that all required behavior passed; `implemented` means reviewed functional
implementation, not just a declaration. Explicit missing API requirements may
be added as `module:Function` IDs with evidence. Evidence for any different
commit is stale and excluded until reviewed again. A new upstream commit may
therefore lower the verified checklist percentage; this is intentionally
conservative and not necessarily a regression.

No combined whole-game percentage is generated. A complete verified checklist
is a qualification milestone, not proof of zero bugs.

## Safety and validation

The workflow uses the upstream checkout as data only and does not execute
upstream code or consume game assets. All repository tokens are read-only.
Python standard-library tests run before report generation. The fork asset
policy permits this exact Python source path while retaining all content checks.
No game-derived reports may be committed or uploaded.

For local title-specific import and shader census commands, see
[Local reporting](#local-reporting) below.

## Local reporting


Use an already existing parent directory, and give every run a new output
directory. The tool refuses to write inside a VIctory checkout or overwrite
a prior snapshot.

```powershell
cargo run --release --locked -p victory -- census D:\MyTitle --progress-dir "$env:TEMP\victory-imports-1"
cargo run --release --locked -p victory -- translate D:\MyTitle --validate --pipelines --jobs 8 --progress-dir "$env:TEMP\victory-shaders-1"
```

- Static import coverage = distinct HLE or host-resolved imports divided by all
  distinct static and delay imports in successfully parsed PE images. DLL
  names are case-normalized and name-import hints are ignored. Unresolved
  imports may be supplied by the title's DLLs at runtime; dynamic imports and
  interface/vtable calls are not measured. Use `--strict` for HLE only.
- Shader coverage counts all decoded programs, including programs refused
  before emission, separately for vertex/pixel, compute and mesh. IR building,
  structurisation and resource tracking are separate metrics. DXIL validation
  and host pipeline creation are optional; attempted counts are reported
  separately. Unknown stages, decode failures and skipped files are disclosed.
  Successful emission, validation or pipeline creation does not demonstrate
  correct GPU output.
- A denominator of zero is N/A, not 100%. Scan failures and unavailable
  validators reduce the confidence or scope of results; do not compare
  incomplete or differently configured runs as if they were identical.

Local reports are subject to [the asset policy](asset-policy.md). Do not place
them in a checkout, Pages site, public artifact, issue or PR.

## History and reproducibility

```powershell
cargo run --locked -p victory -- progress-history "$env:TEMP\victory-imports-1" "$env:TEMP\victory-imports-2" --output "$env:TEMP\victory-import-history"
```

Supply snapshots oldest first. The history dashboard shows every measurement with
its original numerator and denominator, rejects mixed report types, measurement modes and incomplete
snapshots, and preserves the inputs. Use the TSV for longitudinal charts. Do not
infer a regression or improvement from percentages when corpus, host, SDK or
flags have changed. Retain commit SHA, title build identity, corpus selection,
OS, GPU/driver and validator version with your private test records.

## Unmeasured areas

GPU interface semantics, actual draw correctness, texture/audio output,
performance, stability, progression through the story and Series S versus
Series X compatibility still require functional tests. See the status table in
the [README](../README.md) for reported milestones. No combined playability
percentage is generated. The source inventory intentionally does not count
arbitrary TODOs, stub-looking function bodies or source lines as implementation
progress.

## Validation

```powershell
cargo test --locked -p victory progress::tests
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask policy
```

The public workflow never consumes a user's game corpus.
