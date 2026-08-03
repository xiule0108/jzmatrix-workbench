# Governance

## Roles

- The maintainer owns project scope, releases, security decisions, and final acceptance.
- Contributors may propose issues, designs, code, tests, and documentation through GitHub.
- AI agents may assist with implementation and review, but an agent statement is not acceptance evidence.

## Decision records

Changes to the core architecture, adapter contract, permission model, public API, storage migration, or release process require a versioned decision record and independent review.

The project distinguishes four facts that must not be collapsed into one status:

1. a message was written or sent;
2. a target accepted responsibility;
3. an artifact was submitted;
4. an authorized consumer accepted the result.

## Merge policy

- Changes enter `main` through pull requests.
- Automated checks must pass once the implementation baseline exists.
- Security, permission, external-contract, migration, and release changes use the critical review gate described in `docs/REVIEW_POLICY.md`.
- The maintainer may close proposals that depend on undocumented private vendor interfaces or expand the project beyond the current roadmap.

## Release authority

Only the maintainer may publish official releases. A release must identify its source commit, package hashes, supported platforms, known limitations, and recovery boundary.
