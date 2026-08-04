# JZMatrix Workbench

JZMatrix Workbench is a local-first desktop application and CLI for creating and observing collaboration groups across AI coding tools.

The project has a constrained P1-A engineering baseline for macOS development. It is not a supported product release and does not include real adapter integrations.

## Intended product

- Connect supported AI tools without hiding their capability differences.
- Create a collaboration group from a clear goal and a small set of templates.
- Distinguish message delivery, task acceptance, artifact submission, and human acceptance.
- Provide a simple desktop experience for macOS and Windows plus a stable `jzmatrix` CLI.
- Keep user data local by default and avoid private, undocumented vendor storage interfaces.

The first integration candidates are Codex, Claude Code, ZCode, Cursor, VS Code/Copilot, and Zed. Inclusion in a release requires official-interface verification and a real round-trip probe; a candidate name is not a support promise.

## Current status

| Area | Status |
|---|---|
| Product and architecture plan | Frozen for constrained P1-A entry |
| P1-A Rust/CLI/Tauri scaffold | Available on the construction branch |
| Offline fixture and doctor | Implemented as a local baseline |
| Fixed synthetic platform fixture parsing | Implemented for two bundled, manifest-verified fixtures |
| Offline novice flow | Implemented as a clearly labelled demo; no real tool actions |
| macOS and Windows packages | Not released; Windows support is not claimed |
| Real adapters and external writes | Not implemented |

See the [roadmap](docs/ROADMAP.md), [governance](GOVERNANCE.md), and [review policy](docs/REVIEW_POLICY.md).

## Local development

The macOS bootstrap requires Node.js 24.14.0 and npm 11.9.0. It can install the pinned Rust 1.88.0 toolchain into the user-level rustup location with no `sudo`.

```sh
./scripts/bootstrap.sh --install-rust
./scripts/bootstrap.sh
./scripts/doctor.sh
cargo run --offline --locked -p jzmatrix-cli -- --version
cargo run --offline --locked -p jzmatrix-cli -- doctor --json
npm run tauri:dev
npm run tauri:build:app
```

The first command is only needed when the pinned Rust toolchain is not already available. `scripts/doctor.sh` deliberately uses Cargo offline and therefore reports a blocked toolchain if dependencies have not been bootstrapped. The app loads only bundled resources and exposes `doctor` and `offline_demo` in P1-A.

### Fixed synthetic platform fixture parsing

P1-A includes two fully synthetic, transcript-free, secret-free fixtures observed from the 10A probe categories:

- `codex-cli-synthetic-v1`
- `claude-code-synthetic-v1`

They are embedded in the package and verified against `fixtures/platform-events/manifest.json`. The read-only CLI accepts only one of these built-in IDs:

```sh
cargo run --offline --locked -p jzmatrix-cli -- fixture inspect --id codex-cli-synthetic-v1 --json
```

The versioned parser returns event sequence evidence and separate `activity`, `local_written`, `sent_not_confirmed`, `delivered`, `accepted`, and `completed` observations. `completed`, process exit `0`, or a platform status string never upgrades delivery or acceptance. Unknown fields are retained under `extensions`; unknown events, sequence gaps, conflicts, secrets, session-body fields, remote URLs, and absolute paths fail closed or become an explicitly partial/unknown result.

This is a fixed parser and evidence-view capability only. It does not execute an external process, read arbitrary paths, read existing sessions, persist prompt/tool/response bodies, or claim complete Codex/Claude Code protocol coverage. Real adapters, create/send/resume/cancel, Windows support, and S2 evidence remain outside this branch.

`tauri:build:app` produces an unsigned macOS `.app` baseline. Signing, notarization, release installers, and update/rollback infrastructure are outside P1-A.

P1-A does not implement real adapters, create/send/resume/cancel actions, existing-session reads, external Agent writes, shell access, arbitrary SQL, a daemon, an updater, or remote URLs. Windows has a CI verification entry only; this branch does not claim native Windows support or a Windows package release.

## 中文说明

JZMatrix Workbench 是一个本地优先的桌面工具和 CLI，用于连接不同 AI 编码工具、建立分工明确的协作组，并准确呈现接单、执行、提交和验收状态。

核心规划已经通过独立对抗审查，当前仅进入受限 P1-A 工程验证。仓库中的界面只使用明确标注的离线演示数据，CLI 只做本地环境检查和随包合成夹具解析；尚未接入真实工具，也未发布可用安装包。首发目标仍是 macOS 与 Windows，正式发行版不会要求普通用户预装 Node.js、Rust 或数据库。

## License

Licensed under the [Apache License 2.0](LICENSE).
