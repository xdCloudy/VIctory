# Progress tracking

The tracker measures separate things rather than estimating a percentage of a
playable game. Each snapshot has an HTML dashboard, an SVG graphic, a versioned
TSV metrics table, methodology metadata, and a completion marker. No report
contains game binaries, shaders, function names or local paths. **Local reports
are derived from user files and must never be committed or uploaded to public CI.**

## Public source inventory

```powershell
cargo run --locked -p victory -- progress --output "$env:TEMP\victory-source-1"
```

This reads the actual HLE registry, not source-code regexes. It counts registered
exports once per module, not once per DLL pattern. A registered export may be a
stub, and there is no known complete Xbox API denominator. The counts are
inventory, not percentages. The Windows registry includes platform-specific
modules that other hosts omit.

The Progress workflow builds and tests this report from source alone, stores the
commit-specific report as a GitHub Actions artifact, and optionally deploys the
latest main-branch dashboard to GitHub Pages. To enable Pages, a repository owner
must configure Pages to use GitHub Actions and set the repository Actions variable
`VICTORY_PROGRESS_PAGES` to `true`. No game corpus is ever configured in this workflow.
If enabled, the dashboard is at `https://<owner>.github.io/VIctory/` and its SVG at
`https://<owner>.github.io/VIctory/progress.svg`. Artifact retention is governed by
GitHub; download snapshots for permanent history.

## Local title-specific reports

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
its original numerator and denominator, rejects mixed report types and incomplete
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
