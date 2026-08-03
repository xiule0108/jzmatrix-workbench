# JZMatrix Workbench

JZMatrix Workbench is a local-first desktop application and CLI for creating and observing collaboration groups across AI coding tools.

The project is currently in its planning and adversarial-review stage. There is no supported binary or implementation release yet. Product construction will begin only after the architecture plan passes the project’s critical review gate.

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
| Product and architecture plan | In progress |
| Independent critical review | Required before construction |
| Source implementation | Not started |
| macOS and Windows packages | Not available |
| Public API stability | Not established |

See the [roadmap](docs/ROADMAP.md), [governance](GOVERNANCE.md), and [review policy](docs/REVIEW_POLICY.md).

## 中文说明

JZMatrix Workbench 是一个本地优先的桌面工具和 CLI，用于连接不同 AI 编码工具、建立分工明确的协作组，并准确呈现接单、执行、提交和验收状态。

项目目前处于规划与对抗审查阶段，尚未发布可用安装包。核心规划未通过独立审查前，不进入编码施工。首发目标是 macOS 与 Windows；正式发行版不会要求普通用户预装 Node.js、Rust 或数据库。

## License

Licensed under the [Apache License 2.0](LICENSE).
