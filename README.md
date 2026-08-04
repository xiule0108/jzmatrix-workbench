# JZMatrix Workbench

JZMatrix Workbench is a local-first macOS desktop application and CLI under construction for creating and observing collaboration groups across AI coding tools. Windows remains a deferred terminal target and is not a current support claim.

The project has a constrained P1-A engineering baseline for macOS development. It is not a supported product release and does not include real adapter integrations.

## Intended product

- Connect supported AI tools without hiding their capability differences.
- Create a collaboration group from a clear goal and a small set of templates.
- Distinguish message delivery, task acceptance, artifact submission, and human acceptance.
- Provide a simple macOS desktop experience plus a stable `jzmatrix` CLI first; Windows is retained as a later terminal target.
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
| Mac-M1 local facts and one-click groups | Implemented on `main`; local-only and not an external tool action |
| Mac-M2 allowlisted tool discovery | Implemented as a user-triggered version/help snapshot; no sessions or tasks are opened |
| Mac-M4 dry-run permissions | Implemented as a persisted `execution=not_authorized` plan; no external action is available |
| macOS package | Not released; the current line is macOS-only |
| Windows package | Deferred; no current Windows support or release claim |
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

The first command is only needed when the pinned Rust toolchain is not already available. `scripts/doctor.sh` deliberately uses Cargo offline and therefore reports a blocked toolchain if dependencies have not been bootstrapped. The app loads only bundled resources and exposes `doctor`, `offline_demo`, local-only M1 group commands, and the user-triggered M2 allowlisted discovery command.

To inspect the static catalog without probing the machine:

```sh
cargo run --offline --locked -p jzmatrix-cli -- tools catalog --json
```

To perform the explicit Mac-only discovery action, which runs only `--version` and `--help` for the ten named binaries and stores a redacted local snapshot:

```sh
cargo run --offline --locked -p jzmatrix-cli -- tools discover --json
```

The discovery result does not read `~/.codex`, `~/.claude`, session bodies, credentials, or arbitrary paths. An available binary is not a connected Agent and does not grant create, send, resume, cancel, delivery, acceptance, or completion evidence.

After creating a local group, generate a reviewable plan without authorizing any external action:

```sh
cargo run --offline --locked -p jzmatrix-cli -- plan preview --group-id <local-group-id> --json
```

The plan records `create`, `send`, `resume`, and `cancel` as `not_authorized`, persists only a redacted local object, and has no process, network, session, or external-write path. A plan is not an adapter and is not evidence that an external tool accepted or executed anything.

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

P1-A does not implement real adapters or execute create/send/resume/cancel actions, existing-session reads, external Agent writes, shell access, arbitrary SQL, a daemon, an updater, or remote URLs. M4 only persists a `not_authorized` dry-run plan. Windows has a CI verification entry only; this branch does not claim native Windows support or a Windows package release.

## 中文说明

JZMatrix Workbench 是一个本地优先的桌面工具和 CLI，用于连接不同 AI 编码工具、建立分工明确的协作组，并准确呈现接单、执行、提交和验收状态。

核心规划已经通过前序独立审查，当前施工范围根据用户裁定切换为 macOS-only。仓库中的界面在显式保存本地协作组之前只使用明确标注的离线演示数据；Mac-M1 增加本地 SQLite 事实源和一键建组，Mac-M2 增加用户显式触发的白名单版本与帮助快照，Mac-M4 增加可审阅的只读执行计划，但所有外部动作均保持未授权。尚未接入真实工具，也未发布可用安装包。Windows 仍是终局目标，但在 Mac 版本达到公开 Beta 前冻结，不构成当前支持声明。正式发行版不会要求普通用户预装 Node.js、Rust 或数据库。

## License

Licensed under the [Apache License 2.0](LICENSE).
