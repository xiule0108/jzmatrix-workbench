# Vite 6.4.3 security validation

This document records the official advisory branches exercised by `scripts/test-vite-security.mjs`. It is an as-of 2026-08-04 development-security test, not a product network feature. The probe starts Vite on `127.0.0.1` with an operating-system-assigned port, creates only synthetic canaries under a temporary directory, and removes them after the server closes.

## Official advisory coverage

| Official advisory | Affected branch relevant to Vite 6.1.0 | Probe case |
|---|---|---|
| [GHSA-x574-m823-4x7w](https://github.com/advisories/GHSA-x574-m823-4x7w) | `?raw??` and `?import&raw??` bypass of the file allow list | `vite.raw_query_separators_denied` |
| [GHSA-4r4m-qw57-chr8](https://github.com/advisories/GHSA-4r4m-qw57-chr8) | `?import` combined with raw or inline loading | `vite.import_inline_denied` |
| [GHSA-xcj6-pq6g-qj4x](https://github.com/advisories/GHSA-xcj6-pq6g-qj4x) | `.svg` loader and pre-normalization relative-path bypass | `vite.svg_query_denied` |
| [GHSA-356w-63v5-8wf4](https://github.com/advisories/GHSA-356w-63v5-8wf4) | invalid request-target containing `#` | `vite.invalid_request_target_denied` |
| [GHSA-859w-5945-r5v3](https://github.com/advisories/GHSA-859w-5945-r5v3) | slash-dot path under project root | `vite.slash_dot_denied` |
| [GHSA-g4jq-h2w9-997c](https://github.com/advisories/GHSA-g4jq-h2w9-997c) | public-directory symlink and prefix traversal | `vite.public_prefix_traversal_denied` |
| [GHSA-jqfw-vq24-v9c3](https://github.com/advisories/GHSA-jqfw-vq24-v9c3) | HTML handling outside the root and `server.fs` policy | `vite.outside_html_denied` |
| [GHSA-93m4-6634-74q7](https://github.com/advisories/GHSA-93m4-6634-74q7) | trailing backslash on Windows | `windows.backslash_denied` |
| [GHSA-p9ff-h696-f583](https://github.com/advisories/GHSA-p9ff-h696-f583) | HMR WebSocket `vite:invoke` calling `fetchModule` without the HTTP file check | `vite.websocket_fetch_module_denied` |
| [GHSA-4w7w-66w2-5vf9](https://github.com/advisories/GHSA-4w7w-66w2-5vf9) | optimized-dependency source-map path traversal | `vite.optimized_map_parent_traversal_denied`, `windows.optimized_map_backslash_denied` |
| [GHSA-fx2h-pf6j-xcff](https://github.com/advisories/GHSA-fx2h-pf6j-xcff) | NTFS alternate data streams and 8.3 short names | `windows.ads_denied`, `windows.short_name_denied` |
| [GHSA-v6wh-96g9-6wx3](https://github.com/advisories/GHSA-v6wh-96g9-6wx3) | launch-editor UNC path causing Windows NTLM authentication | `windows.unc_launch_editor_denied` |
| [GHSA-67mh-4wv8-2f99](https://github.com/advisories/GHSA-67mh-4wv8-2f99) | esbuild development server cross-origin response exposure through 0.24.2 | `dependency.esbuild_floor` plus `npm audit --json` |

The Windows cases require `windows-latest` and an NTFS temporary volume. They report `skipped` on other operating systems instead of claiming a pass. The 8.3 case obtains the real NTFS short path; if automatic short-name generation is unavailable, it attempts to assign a short alias to the temporary canary only. Failure to obtain an actual alias blocks the test.

## Instrument controls

- `control.safe_resource` must return a known safe marker with HTTP 200.
- `control.default_deny` must reject a normal `.env?raw` request and must not return the denied marker.
- The WebSocket case must complete the real Vite HMR handshake and receive an error response for `fetchModule`.
- The UNC case points only to `\\127.0.0.1\\jzmatrix-no-share`. Before loading Vite, the harness installs filesystem and child-process interceptors. A local-file launch request must exercise both interceptors as a positive control; the following UNC request must exercise neither. The case also checks that the installed Vite guard precedes both operations. It never names a public SMB host.
- Forward- and backslash-traversal requests use raw loopback TCP so URL normalization in a client cannot erase the advisory input before Vite receives it.

Run the probe with:

```sh
npm run test:security:vite
```

The command prints one JSON object with per-case status. A failed Windows prerequisite is a failure, not a skip.
