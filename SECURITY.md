# Security Policy

## Supported versions

There is no supported release yet. Security support begins with the first signed public beta.

## Reporting a vulnerability

Please use GitHub private vulnerability reporting for this repository. Do not open a public issue containing secrets, private conversation content, local paths, proof-of-concept credentials, or exploitable details.

Include the affected version or commit, operating system, impact, reproduction steps, and any suggested mitigation. The maintainer will acknowledge a valid report and coordinate disclosure after a fix is available.

## Security principles

- Local-first is not treated as equivalent to secure.
- Read access, create access, and send access are separate permissions.
- Credentials must use operating-system secret storage.
- Undocumented private vendor storage is outside the supported integration boundary.
- Logs and exports must redact credentials, stable personal identifiers, and user-specific absolute paths.
- Official packages require signatures, hashes, an SBOM, provenance, and a tested recovery path.
