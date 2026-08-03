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
| macOS and Windows packages | Not released; Windows support is not claimed |
| Real adapters and external writes | Not implemented |

See the [roadmap](docs/ROADMAP.md), [governance](GOVERNANCE.md), and [review policy](docs/REVIEW_POLICY.md).

## Local development

The macOS bootstrap requires Node.js 24.14.0 and npm 11.9.0. It can install the pinned Rust 1.85.1 toolchain into the user-level rustup location with no `sudo`.

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

`tauri:build:app` produces an unsigned macOS `.app` baseline. Signing, notarization, release installers, and update/rollback infrastructure are outside P1-A.

P1-A does not implement real adapters, create/send/resume/cancel actions, existing-session reads, external Agent writes, shell access, arbitrary SQL, a daemon, an updater, or remote URLs. Windows has a CI verification entry only; this branch does not claim native Windows support or a Windows package release.

## 中文说明

JZMatrix Workbench 是一个本地优先的桌面工具和 CLI，用于连接不同 AI 编码工具、建立分工明确的协作组，并准确呈现接单、执行、提交和验收状态。

项目目前处于规划与对抗审查阶段，尚未发布可用安装包。核心规划未通过独立审查前，不进入编码施工。首发目标是 macOS 与 Windows；正式发行版不会要求普通用户预装 Node.js、Rust 或数据库。

## License

Licensed under the [Apache License 2.0](LICENSE).
