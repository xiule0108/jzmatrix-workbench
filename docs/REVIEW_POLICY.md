# Review Policy

Implementation self-review and independent review are different controls. The model or person producing a change cannot provide the only acceptance evidence for that change.

## Review levels

| Level | Examples | Review route |
|---|---|---|
| S — critical | Overall plan, core architecture, permissions, public API contracts, irreversible migrations, public releases | Official Claude Fable adversarial review, deterministic verification, and maintainer adjudication. Conditional approval does not authorize work until every blocker has closure evidence. |
| A — important | Adapter architecture, major modules, cross-platform implementation | Independent Kimi K3 and GLM 5.2 reviews where available, plus tests; escalate material disagreements to level S. |
| B — bounded | A single adapter, local UI flow, documentation, test coverage | One independent Kimi K3 or GLM 5.2 review, rotated by task fit, plus deterministic checks. |
| C — mechanical | Formatting, schemas, lockfiles, hashes, build matrices | Scripts and CI. DeepSeek V4 Flash may provide a fast supplemental scan when its channel is verified, but it cannot approve a critical gate. |

The designated construction model is GPT-5.6 LUNA with maximum reasoning on the priority service tier. Its own checks do not replace independent review.

## Required record

Every level S record must identify the reviewed material and revision, model and access channel, decision, blockers, closure evidence, deterministic verification, and final maintainer adjudication.
