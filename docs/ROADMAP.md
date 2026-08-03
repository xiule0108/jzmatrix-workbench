# Roadmap

The roadmap is evidence-gated. Dates do not override a failed gate.

## P0 — Planning and independent review

- Verify official integration boundaries for candidate tools.
- Freeze the first adapter capability contract.
- Define the novice product flow and cross-platform release baseline.
- Obtain an explicit critical-review decision.

No implementation construction is authorized until P0 is closed.

## P1 — Installable vertical slice

- New Rust core, SQLite migrations, CLI, Tauri shell, and synthetic demo mode.
- Three primary paths: connect tools, create a collaboration group, view progress.
- macOS and Windows CI builds and clean-install smoke tests.

## P2 — Capability-driven adapters

- Adapter SDK and machine-readable capability manifests.
- Read-only or package-generation integrations first.
- Real round-trip probes before enabling create, resume, send, or receipt actions.

## P3 — One-click collaboration groups

- Templates, preview, open task package, partial-failure recovery, and CLI parity.

## P4 — Delivery and acceptance

- Separate responsibility acceptance, execution evidence, artifact submission, and consumer acceptance.

## P5 — Signed public beta

- Signed and notarized packages, checksums, SBOM, provenance, update and rollback, backup and recovery exercises.

## Research track

The usable workbench may continue even if experiments show that a general multi-agent coordination framework has no durable advantage. Framework work requires reproducible evidence beyond cross-tool integration and UI improvements.
