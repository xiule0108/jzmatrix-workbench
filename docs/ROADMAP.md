# Roadmap

The roadmap is evidence-gated. Dates do not override a failed gate.

## P0 — Planning and independent review

- Verify official integration boundaries for candidate tools.
- Freeze the first adapter capability contract.
- Define the novice product flow and cross-platform release baseline.
- Obtain an explicit critical-review decision.

No implementation construction is authorized until P0 is closed.

## P1 — macOS installable vertical slice

- New Rust core, SQLite migrations, CLI, Tauri shell, and synthetic demo mode.
- Three primary paths: connect tools, create a collaboration group, view progress.
- Mac-M1 local fact source and one-click group creation are implemented first; the group remains local-only and records `local_written` without claiming delivery, acceptance, or completion.
- Mac-M2 allowlisted discovery is implemented as a user-triggered `--version`/`--help` snapshot; it records only redacted metadata and never reads sessions or starts Agent tasks.
- Mac-M4 dry-run permissions are implemented as a local persisted plan. `create`, `send`, `resume`, and `cancel` remain `not_authorized`; no external process, network, session read, or write is exposed.
- macOS arm64 first, then macOS x86_64 clean-install smoke tests.
- Windows CI artifacts remain historical evidence only; Windows construction and release are deferred until the Mac public-beta Gate closes.

## P2 — macOS capability-driven adapters

- Adapter SDK and machine-readable capability manifests.
- Read-only or package-generation integrations first.
- Real round-trip probes before enabling create, resume, send, or receipt actions.

## P3 — macOS one-click collaboration groups

- Templates, preview, open task package, partial-failure recovery, and CLI parity.

## P4 — Delivery and acceptance

- Separate responsibility acceptance, execution evidence, artifact submission, and consumer acceptance.

## P5 — Signed macOS public beta

- Signed and notarized packages, checksums, SBOM, provenance, update and rollback, backup and recovery exercises.
- Windows signing, runtime evidence, and release are a later terminal Gate, not part of this phase.

## Research track

The usable workbench may continue even if experiments show that a general multi-agent coordination framework has no durable advantage. Framework work requires reproducible evidence beyond cross-tool integration and UI improvements.
