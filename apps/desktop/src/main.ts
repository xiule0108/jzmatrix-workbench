import { invoke } from "@tauri-apps/api/core";
import type {
  CliResponse,
  CollaborationGroup,
  DoctorData,
  OfflineDemo,
  OfflineDemoGroup,
  ToolDiscoveryResult,
  ToolDiscoverySnapshot,
} from "@jzmatrix/protocol";
import "./styles.css";

type Screen = 1 | 2 | 3;
type TemplateId = "compact" | "research" | "product" | "content";
type RuntimeSource = "browser-fallback" | "tauri";
type EvidenceId =
  | "connect"
  | "package"
  | "window"
  | "activity"
  | "progress"
  | "local-written"
  | "delivery"
  | "demo-boundary";

interface RuntimeInfo {
  source: RuntimeSource;
  demo: OfflineDemo | null;
  demoStatus: "loading" | "ready" | "blocked";
  doctorStatus: string;
  doctorObservedAt: string | null;
  doctorSummary: string;
  doctorDetail: string;
}

interface PageState {
  screen: Screen;
  goal: string;
  template: TemplateId;
  created: boolean;
  creating: boolean;
  localGroup: CollaborationGroup | null;
  idempotencyKey: string;
  discovering: boolean;
  toolDiscovery: ToolDiscoverySnapshot | null;
  discoveryError: string | null;
  formError: string | null;
  evidence: EvidenceId | null;
  runtime: RuntimeInfo;
}

interface EvidenceRecord {
  title: string;
  status: string;
  source: string;
  observed: string;
  canConfirm: string;
  cannotConfirm: string;
  next: string;
}

const app = document.querySelector<HTMLElement>("#app") ?? (() => {
  throw new Error("页面容器不存在");
})();
const hasTauri = "__TAURI_INTERNALS__" in window;

// 这是浏览器开发预览的明确回退，只用于演示页面状态；桌面壳中改由 offline_demo 提供。
const browserDemoFallback: OfflineDemo = {
  contract: "jzmatrix.offline-demo",
  version: "1.0.0",
  data_source: "demo",
  title: "离线演示",
  groups: [
    {
      id: "0190a000-0000-7000-8000-000000000001",
      goal: "整理一份可复核的离线工作包",
      status: "demo_ready",
      roles: ["研究", "复核"],
      events: [
        { kind: "work_package_prepared", status: "local_written" },
        { kind: "external_send", status: "not_run" },
      ],
    },
  ],
  optional_agent: {
    status: "not_run",
    reason: "离线演示不需要外部 Agent",
  },
};

const state: PageState = {
  screen: 1,
  goal: "",
  template: "compact",
  created: false,
  creating: false,
  localGroup: null,
  idempotencyKey: "",
  discovering: false,
  toolDiscovery: null,
  discoveryError: null,
  formError: null,
  evidence: null,
  runtime: {
    source: hasTauri ? "tauri" : "browser-fallback",
    demo: hasTauri ? null : browserDemoFallback,
    demoStatus: hasTauri ? "loading" : "ready",
    doctorStatus: hasTauri ? "正在读取" : "开发预览",
    doctorObservedAt: null,
    doctorSummary: hasTauri ? "正在等待桌面壳返回本地检查结果" : "浏览器预览未连接桌面壳",
    doctorDetail: hasTauri
      ? "只调用已有的 Rust doctor，不会执行工具探针。"
      : "浏览器预览使用内置离线演示回退，不读取本机。",
  },
};

let lastFocusedElement: HTMLElement | null = null;

const evidenceRecords: Record<EvidenceId, EvidenceRecord> = {
  connect: {
    title: "工具连接能力",
    status: "未检查 / 只读能力待确认",
    source: "Mac-M2 固定工具白名单与用户触发探针",
    observed: "未点击“检查 Mac 工具”前不读取本机；点击后只保存版本摘要、能力标记和不可用原因",
    canConfirm: "可以确认某个具名二进制是否能在限定时间内返回 --version/--help",
    cannotConfirm: "不能确认工具能建立工作窗口、读取会话、发送消息或完成任务",
    next: "先查看能力快照，再在后续独立 Gate 中讨论真实只读适配器",
  },
  package: {
    title: "协作组工作包",
    status: "分工已准备好（演示预览）",
    source: "offline_demo fixture / 页面内存状态",
    observed: "演示时间线；点击建立后生成预览",
    canConfirm: "目标和默认分工已在当前页面预览",
    cannotConfirm: "不能确认工具里的真实工作窗口已经建立",
    next: "查看进展，或在工具中手动使用交接说明",
  },
  window: {
    title: "工具工作窗口",
    status: "尚未建立",
    source: "当前 P1-A 仅允许离线演示和只读检查",
    observed: "演示时间线",
    canConfirm: "本页面没有执行创建工作窗口的动作",
    cannotConfirm: "不能把工作包预览当成真实工具窗口",
    next: "等待后续真实创建能力通过独立确认",
  },
  activity: {
    title: "最近有动静",
    status: "最近有动静",
    source: "演示成员状态映射 / 活动字段",
    observed: "演示时间线，不代表本机时间",
    canConfirm: "只能说明工作文件或状态记录有变化",
    cannotConfirm: "不能说明任务正在执行，更不能说明已经完成",
    next: "等待正式进度记录，或打开工具核对",
  },
  progress: {
    title: "正式进度记录",
    status: "暂时没有可核对的进度记录",
    source: "offline_demo fixture 未提供正式任务进度字段",
    observed: "演示时间线",
    canConfirm: "当前没有可独立核对的正式进度记录",
    cannotConfirm: "不能用最近活动、标题或时间戳补出进度",
    next: "打开工具查看正式记录",
  },
  "local-written": {
    title: "本机消息记录",
    status: "已写入本机消息记录",
    source: "offline_demo fixture / work_package_prepared",
    observed: "演示时间线",
    canConfirm: "本机演示记录中存在一条写入事件",
    cannotConfirm: "不能确认任何其他人已经看到这条消息",
    next: "连接并获得真实送达回执后再判断",
  },
  delivery: {
    title: "消息送达",
    status: "尚未确认送达",
    source: "offline_demo fixture / external_send 未运行",
    observed: "演示时间线",
    canConfirm: "发送动作没有在真实工具中执行",
    cannotConfirm: "不能确认对方是否收到消息",
    next: "不要把本机写入当成送达；后续在工具中手动核对",
  },
  "demo-boundary": {
    title: "演示数据边界",
    status: "演示数据",
    source: "offline_demo fixture；浏览器预览为明确标注的内置回退",
    observed: "页面打开时加载，演示时间线固定",
    canConfirm: "这组数据不会读取本机，也不会联系真实工具或人员",
    cannotConfirm: "不能把演示成员、时间、消息或进度当成真实状态",
    next: "点击“连接真实工具”回到连接页，不会迁移演示结果",
  },
};

const templateOptions: Array<{
  id: TemplateId;
  label: string;
  description: string;
  roles: string;
}> = [
  {
    id: "compact",
    label: "简单完成",
    description: "适合先把一件事讲清楚并得到检查",
    roles: "统筹 · 执行 · 独立检查",
  },
  {
    id: "research",
    label: "查资料并核对",
    description: "增加资料与复核分工，适合事实核验",
    roles: "统筹 · 资料 · 独立检查",
  },
  {
    id: "product",
    label: "从需求做到成品",
    description: "覆盖需求、实现和发布前检查",
    roles: "统筹 · 执行 · 验证 · 发布前检查",
  },
  {
    id: "content",
    label: "写作与成品",
    description: "适合资料、写作、视觉和发布前检查",
    roles: "资料 · 写作 · 成品整理 · 检查",
  },
];

const toolCatalog: Array<{
  id: string;
  name: string;
  description: string;
  scope: string;
}> = [
  { id: "codex_cli", name: "Codex CLI", description: "OpenAI 的本地编码助手命令行", scope: "只检查可执行文件和帮助信息" },
  { id: "claude_code_cli", name: "Claude Code", description: "Anthropic 的本地编码助手命令行", scope: "只检查可执行文件和帮助信息" },
  { id: "cursor_agent", name: "Cursor Agent", description: "Cursor 的命令行 Agent 入口", scope: "只检查本机 CLI，不连接后台任务" },
  { id: "copilot_cli", name: "GitHub Copilot CLI", description: "GitHub Copilot 的命令行入口", scope: "只检查本机 CLI，不读取远程会话" },
  { id: "zed", name: "Zed", description: "Zed 编辑器命令行入口", scope: "只检查命令行入口，不打开编辑器" },
  { id: "zcode", name: "ZCode", description: "Z.AI ZCode 桌面入口", scope: "只检查命令行入口，不读取桌面会话" },
  { id: "vscode", name: "VS Code", description: "VS Code 命令行入口", scope: "只检查编辑器 CLI，不读取 Copilot 会话" },
  { id: "opencode", name: "OpenCode", description: "OpenCode 命令行与 ACP 样本", scope: "只检查 CLI，不启动 HTTP 或 ACP 服务" },
  { id: "cline", name: "Cline", description: "Cline 命令行与 ACP 样本", scope: "只检查 CLI，不创建任务或 ACP 会话" },
  { id: "aider", name: "Aider", description: "Aider 命令行对照工具", scope: "只检查 CLI，不读取历史或修改 Git" },
];

function escapeHtml(value: unknown): string {
  return String(value).replace(/[&<>"']/g, (character) => {
    const entities: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#39;",
    };
    return entities[character] ?? character;
  });
}

function statusTag(label: string, tone: "good" | "notice" | "unknown" | "demo", icon: string): string {
  return `<span class="status-tag status-tag--${tone}"><span class="status-icon" aria-hidden="true">${icon}</span>${escapeHtml(label)}</span>`;
}

function currentGroup(): OfflineDemoGroup | null {
  if (state.localGroup) {
    const local = state.localGroup;
    const localWritten = local.facts.find((fact) => fact.kind === "local_written");
    return {
      id: local.id,
      goal: local.goal,
      status: local.status,
      roles: local.roles.map((role) => role.label),
      events: [
        { kind: "work_package_prepared", status: localWritten?.state === "observed" ? "local_written" : "unknown" },
        { kind: "external_send", status: "not_run" },
      ],
    };
  }
  return state.runtime.demo?.groups[0] ?? null;
}

function isLocalGroup(): boolean {
  return state.localGroup !== null;
}

function selectedTemplate(): (typeof templateOptions)[number] {
  return templateOptions.find((option) => option.id === state.template) ?? templateOptions[0];
}

function observedLabel(): string {
  if (state.runtime.source === "browser-fallback") {
    return "演示时间线，不代表本机时间";
  }
  return state.runtime.doctorObservedAt
    ? `桌面壳最近读取于 ${formatDate(state.runtime.doctorObservedAt)}`
    : "桌面壳尚未返回观察时间";
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function safeDemoGroup(): OfflineDemoGroup {
  return (
    currentGroup() ?? {
      id: "demo-unavailable",
      goal: "暂无演示目标",
      status: "blocked",
      roles: [],
      events: [],
    }
  );
}

function renderHeader(): string {
  const navItems = [
    { step: 1 as Screen, label: "连接工具", detail: "先看范围" },
    { step: 2 as Screen, label: "建立协作组", detail: "准备分工" },
    { step: 3 as Screen, label: "看进展", detail: "核对状态" },
  ];

  return `
    <header class="app-header">
      <div class="brand-lockup" aria-label="介子九维协作工作台">
        <span class="brand-mark" aria-hidden="true">九</span>
        <span class="brand-name">介子九维 <em>协作工作台</em></span>
      </div>
      <div class="header-state" aria-label="当前数据与连接状态">
        <span class="mode-badge" data-testid="mode-badge"><span class="mode-dot" aria-hidden="true"></span>${isLocalGroup() ? "本机记录" : "演示数据"}</span>
        <span class="connection-badge"><span class="status-icon" aria-hidden="true">○</span>${toolDiscoverySummary()}</span>
      </div>
    </header>
    <div class="demo-banner" data-testid="demo-banner" role="status">
      <span class="banner-mark" aria-hidden="true">${isLocalGroup() ? "本" : "示"}</span>
      <p><strong>${isLocalGroup() ? "本机记录" : "演示数据"}</strong><span>${isLocalGroup() ? "协作组已写入本机事实源，但不会打开真实工具或发送消息。" : "以下内容是示例，不是你电脑上的真实状态。演示不会打开真实工具，也不会发送消息。"}</span></p>
      ${state.screen === 1 ? "" : '<button class="text-button text-button--light" type="button" data-action="back-to-connect">连接真实工具</button>'}
    </div>
    <nav class="planning-strip" id="planning-strip" aria-label="三步流程">
      ${navItems
        .map((item) => {
          const isCurrent = state.screen === item.step;
          const isDone = state.screen > item.step;
          const canVisit = item.step === 1 || (item.step === 2 && state.runtime.demoStatus === "ready") || (item.step === 3 && state.created);
          return `
            <button class="step-item ${isCurrent ? "is-current" : ""} ${isDone ? "is-done" : ""}" type="button" data-action="go-step" data-step="${item.step}" ${canVisit ? "" : "disabled"} ${isCurrent ? 'aria-current="step"' : ""}>
              <span class="step-number" aria-hidden="true">${isDone ? "✓" : item.step}</span>
              <span class="step-copy"><strong>${item.label}</strong><small>${item.detail}</small></span>
            </button>`;
        })
        .join('<span class="step-connector" aria-hidden="true"></span>')}
    </nav>`;
}

function discoveryResult(id: string): ToolDiscoveryResult | null {
  return state.toolDiscovery?.tools.find((tool) => tool.id === id) ?? null;
}

function discoveryStatus(result: ToolDiscoveryResult | null): { label: string; tone: "good" | "notice" | "unknown"; icon: string } {
  if (!result) return { label: "未检查", tone: "unknown", icon: "○" };
  if (result.status === "available" && result.help_status === "pass") {
    return { label: result.version ? `可用 · ${result.version}` : "可用 · 版本未解析", tone: "good", icon: "✓" };
  }
  if (result.status === "available") return { label: "已找到 · 能力未完整确认", tone: "notice", icon: "!" };
  if (result.status === "not_found") return { label: "未找到", tone: "unknown", icon: "○" };
  if (result.status === "timed_out") return { label: "检查超时", tone: "unknown", icon: "!" };
  return { label: "检查受限", tone: "unknown", icon: "!" };
}

function renderDiscoveryStatus(id: string): string {
  const status = discoveryStatus(discoveryResult(id));
  return statusTag(status.label, status.tone, status.icon);
}

function toolDiscoverySummary(): string {
  if (state.discovering) return "正在检查本机工具";
  if (state.toolDiscovery) {
    const available = state.toolDiscovery.tools.filter((tool) => tool.status === "available").length;
    return `已检查 ${available}/${state.toolDiscovery.tools.length} 个入口`;
  }
  return "尚未确认连接";
}

function renderScreenOne(): string {
  return `
    <section class="screen screen--connect" data-testid="screen-connect" aria-labelledby="connect-title">
      <div class="screen-intro">
        <p class="screen-kicker">第一步 <span>·</span> 连接工具</p>
        <h1 id="connect-title">先连接你要用的工具</h1>
        <p class="intro-copy">工作台只读取工作状态，不读取对话正文，也不会未经确认替你发送消息。</p>
      </div>
      <div class="notice-card notice-card--quiet">
        <span class="notice-symbol" aria-hidden="true">只</span>
        <div>
          <strong>只检查本机入口</strong>
          <p>点击后只对固定白名单二进制执行 <code>--version</code> 和 <code>--help</code>。不会读取配置、会话正文或凭据，也不会启动 Agent 任务。</p>
        </div>
        <button class="icon-button" type="button" aria-label="查看连接依据" data-action="open-evidence" data-evidence="connect">?</button>
      </div>
      <div class="trust-points" aria-label="读取范围说明">
        <div class="trust-point"><span aria-hidden="true">01</span><p><strong>会读取</strong>工具是否可用、脱敏后的项目位置和最近活动。</p></div>
        <div class="trust-point"><span aria-hidden="true">02</span><p><strong>不会读取对话正文</strong>、凭据内容或未列出的文件。</p></div>
        <div class="trust-point"><span aria-hidden="true">03</span><p><strong>不会自动做</strong>创建窗口、发送消息、恢复或取消任务。</p></div>
      </div>
      <div class="section-heading">
        <div><p class="eyebrow">可选工具</p><h2>先看清每个工具能提供什么</h2></div>
        <span class="heading-note">${toolDiscoverySummary()}</span>
      </div>
      <div class="tool-list">
        ${toolCatalog
          .map(
            (tool, index) => `
              <article class="tool-card" data-testid="tool-card-${index + 1}">
                <div class="tool-logo" aria-hidden="true">${tool.name.slice(0, index < 2 ? 2 : 1)}</div>
                <div class="tool-main">
                  <div class="tool-title-row"><h3>${tool.name}</h3>${renderDiscoveryStatus(tool.id)}</div>
                  <p class="tool-description">${tool.description}</p>
                  <p class="tool-scope"><span>可见范围</span>${tool.scope}</p>
                </div>
                <span class="tool-binary">${tool.id}</span>
              </article>`,
          )
          .join("")}
      </div>
      ${state.discoveryError ? `<p class="input-error" role="alert">${escapeHtml(state.discoveryError)}</p>` : ""}
      <p class="under-card-note"><span aria-hidden="true">↳</span>${hasTauri ? "检查只在你点击后发生；结果只保留版本摘要、能力标记和不可用原因。" : "浏览器开发预览不会读取本机；请在 Mac 桌面壳中点击检查。"}</p>
      <div class="screen-actions screen-actions--connect">
        <button class="button button--outline" type="button" data-action="discover-tools" ${hasTauri && !state.discovering ? "" : "disabled"}><span>${state.discovering ? "正在检查…" : "检查 Mac 工具"}</span><span aria-hidden="true">⌕</span></button>
        <button class="button button--primary" type="button" data-action="enter-demo"><span>先看一个演示</span><span aria-hidden="true">↗</span></button>
        <button class="button button--quiet" type="button" data-action="open-evidence" data-evidence="demo-boundary">查看演示边界</button>
      </div>
    </section>`;
}

function renderTemplateCard(option: (typeof templateOptions)[number]): string {
  const selected = state.template === option.id;
  return `
    <button class="template-card ${selected ? "is-selected" : ""}" type="button" role="radio" aria-checked="${selected}" data-action="select-template" data-template="${option.id}">
      <span class="radio-mark" aria-hidden="true">${selected ? "●" : "○"}</span>
      <span class="template-copy"><strong>${option.label}</strong><small>${option.description}</small><em>${option.roles}</em></span>
      ${option.id === "compact" ? '<span class="recommend-label">推荐</span>' : ""}
    </button>`;
}

function renderScreenTwo(): string {
  const selected = selectedTemplate();
  const canBuild = state.goal.trim().length > 0 && state.runtime.demoStatus === "ready";
  const localMode = state.runtime.source === "tauri";
  return `
    <section class="screen screen--group" data-testid="screen-group" aria-labelledby="group-title">
      <div class="screen-intro screen-intro--split">
        <div><p class="screen-kicker">第二步 <span>·</span> 建立协作组</p><h1 id="group-title">用一句话说清楚你要完成什么</h1><p class="intro-copy">工作台先给出一套默认分工。${localMode ? "保存到本机事实源，不启动外部工具。" : "当前只生成演示预览，不创建真实工作窗口。"}</p></div>
        <div class="source-stamp"><span class="source-stamp__label">当前来源</span><strong>${localMode ? "本机事实源" : "离线演示"}</strong><small>${escapeHtml(state.runtime.source === "browser-fallback" ? "浏览器内置回退" : localMode ? "Rust matrix-core" : "Rust offline_demo")}</small></div>
      </div>
      <div class="goal-card">
        <label for="goal-input">协作目标</label>
        <textarea id="goal-input" rows="3" maxlength="120" placeholder="例如　核验一项新政策，并形成一份可发布的报告">${escapeHtml(state.goal)}</textarea>
        <div class="input-foot"><span>${state.formError ? `<span class="input-error" id="goal-error" role="alert">${escapeHtml(state.formError)}</span>` : "一句话就够，不需要技术名词"}</span><span id="goal-count">${state.goal.length}/120</span></div>
      </div>
      <div class="section-heading section-heading--compact"><div><p class="eyebrow">协作方式</p><h2>选一套分工，之后仍可调整</h2></div><span class="heading-note">默认选中推荐项</span></div>
      <div class="template-grid" role="radiogroup" aria-label="协作方式">
        ${templateOptions.map(renderTemplateCard).join("")}
      </div>
      <div class="preview-grid">
        <section class="preview-card preview-card--happens" aria-labelledby="happens-title"><div class="preview-heading"><span class="preview-index">A</span><h2 id="happens-title">将发生</h2></div><ul><li>保存你的目标和协作方式</li><li>生成分工说明与本地事实记录</li><li>为每个状态保留可核对的边界</li></ul></section>
        <section class="preview-card preview-card--not" aria-labelledby="not-title"><div class="preview-heading"><span class="preview-index">B</span><h2 id="not-title">不会发生</h2></div><ul><li>不会把最近有动静标成已完成</li><li>不会创建工具里的真实工作窗口</li><li>不会读取对话正文或发送消息</li></ul></section>
      </div>
      <div class="screen-actions screen-actions--group">
        <button class="button button--primary" type="button" data-action="build-group" ${canBuild && !state.creating ? "" : "disabled"}><span>${state.creating ? "正在保存…" : localMode ? "保存本地协作组" : "建立协作组（演示预览）"}</span><span aria-hidden="true">→</span></button>
        <button class="button button--quiet" type="button" data-action="go-step" data-step="1">回到连接工具</button>
        <p class="action-caption">${canBuild ? `当前分工：${escapeHtml(selected.label)} · ${escapeHtml(selected.roles)}` : "先写下一句话，按钮就会亮起"}</p>
      </div>
    </section>`;
}

function roleDisplay(role: string, index: number): string {
  if (role === "研究") return "资料与执行";
  if (role === "复核") return "独立检查";
  return role || (index === 0 ? "资料与执行" : "独立检查");
}

function renderMemberCards(group: OfflineDemoGroup): string {
  const roles = group.roles.length > 0 ? group.roles : ["研究", "复核"];
  const memberSource = isLocalGroup() ? "本机" : "演示";
  const activitySource = isLocalGroup() ? "本机事实源 · 只说明本地写入" : "演示时间线 · 只说明文件有变化";
  return roles
    .slice(0, 3)
    .map((role, index) => {
      const label = roleDisplay(role, index);
      const hasActivity = index === 0;
      return `
        <article class="member-card" data-testid="member-card-${index + 1}">
          <div class="member-top"><span class="member-avatar" aria-hidden="true">${index === 0 ? "资" : "检"}</span><div><h3>${label}</h3><small>分工 ${index + 1}</small></div><span class="member-state">${memberSource}</span></div>
          <div class="member-fact"><span>正式进度</span><strong class="fact-unknown">暂时没有可核对的进度记录</strong><button class="inline-evidence" type="button" data-action="open-evidence" data-evidence="progress">查看依据</button></div>
          <div class="member-fact"><span>最近活动</span><strong>${hasActivity ? "最近有动静" : "尚未看到活动记录"}</strong><small>${hasActivity ? activitySource : "没有记录，不代表没有工作"}</small></div>
          <p class="member-next"><span aria-hidden="true">↳</span>${hasActivity ? "等待正式进度记录，不能把活动当完成" : "等待可核对的工作状态"}</p>
        </article>`;
    })
    .join("");
}

function renderMessageCard(kind: "local-written" | "delivery"): string {
  const isLocal = kind === "local-written";
  const localMode = isLocalGroup();
  const label = isLocal ? "协作组工作包说明" : "给独立检查的交接提醒";
  const status = isLocal ? "已写入本机消息记录" : "尚未确认送达";
  const tone = isLocal ? "notice" : "unknown";
  const icon = isLocal ? "↳" : "○";
  const internalState = isLocal ? "local_written" : "sent_not_confirmed";
  return `
    <article class="message-card" data-testid="message-${kind}" data-state="${internalState}">
      <div class="message-top"><span class="message-icon" aria-hidden="true">${isLocal ? "写" : "问"}</span><div><h3>${label}</h3><small>${localMode ? "本机记录 · 不联系任何人" : "演示消息 · 不联系任何人"}</small></div>${statusTag(status, tone, icon)}</div>
      <p class="message-copy">${isLocal ? localMode ? "目标和分工已写入本机事实源。" : "目标和分工已留在本机演示记录中。" : "消息没有执行真实发送，因此无法判断对方是否看到。"}</p>
      <div class="message-actions"><button class="inline-evidence" type="button" data-action="open-evidence" data-evidence="${kind}">查看依据</button><details class="technical-inline"><summary>技术状态</summary><code>${internalState}</code></details></div>
    </article>`;
}

function renderProgressRail(): string {
  const localMode = isLocalGroup();
  const steps = [
    { title: "目标已保存", detail: localMode ? "本机事实源" : "演示页面内", tone: "good", icon: "✓" },
    { title: "分工已准备", detail: localMode ? "本地协作组" : "协作组预览", tone: "good", icon: "✓" },
    { title: "工具工作窗口", detail: "尚未建立", tone: "unknown", icon: "○" },
    { title: "正式进度", detail: "尚未确认", tone: "unknown", icon: "○" },
    { title: "消息送达", detail: "尚未确认", tone: "unknown", icon: "○" },
  ];
  return `<ol class="progress-rail" aria-label="协作组状态轨道">${steps
    .map(
      (step) => `<li class="progress-node progress-node--${step.tone}"><span class="progress-dot" aria-hidden="true">${step.icon}</span><span><strong>${step.title}</strong><small>${step.detail}</small></span></li>`,
    )
    .join("")}</ol>`;
}

function renderScreenThree(): string {
  const group = safeDemoGroup();
  const goal = state.goal || group.goal;
  const packageEvent = group.events.find((event) => event.kind === "work_package_prepared");
  const packageIsWritten = packageEvent?.status === "local_written";
  const localMode = isLocalGroup();
  return `
    <section class="screen screen--progress" data-testid="screen-progress" aria-labelledby="progress-title">
      <div class="screen-intro screen-intro--progress">
        <div><p class="screen-kicker">第三步 <span>·</span> 看进展</p><h1 id="progress-title">看懂现在发生了什么</h1><p class="intro-copy">把最近有动静、正式进度、工作包、工具窗口和消息送达分别看。</p></div>
        <div class="group-summary"><span class="group-summary__label">${localMode ? "当前本地协作组" : "当前演示协作组"}</span><strong>${escapeHtml(goal)}</strong><button class="inline-evidence" type="button" data-action="open-evidence" data-evidence="${localMode ? "package" : "demo-boundary"}">${localMode ? "查看本机依据" : "为什么是演示？"}</button></div>
      </div>
      ${renderProgressRail()}
      <div class="state-strip" aria-label="关键状态分开显示">
        <div class="state-cell"><span>协作组工作包</span>${statusTag(packageIsWritten ? "已准备（演示）" : "尚未确认", packageIsWritten ? "demo" : "unknown", packageIsWritten ? "✓" : "○")}<button type="button" class="inline-evidence" data-action="open-evidence" data-evidence="package">查看依据</button></div>
        <div class="state-cell"><span>工具工作窗口</span>${statusTag("尚未建立", "unknown", "○")}<button type="button" class="inline-evidence" data-action="open-evidence" data-evidence="window">查看依据</button></div>
        <div class="state-cell"><span>消息本机写入</span>${statusTag("已写入本机消息记录", "notice", "↳")}<span class="state-code">local_written</span></div>
        <div class="state-cell"><span>消息送达</span>${statusTag("尚未确认送达", "unknown", "○")}<span class="state-code">sent_not_confirmed</span></div>
      </div>
      <div class="progress-layout">
        <section class="panel-section" aria-labelledby="members-title"><div class="section-heading"><div><p class="eyebrow">成员与进度</p><h2 id="members-title">有正式记录才进入进度轨道</h2></div><button class="icon-button" type="button" aria-label="查看正式进度依据" data-action="open-evidence" data-evidence="progress">?</button></div><div class="member-list">${renderMemberCards(group)}</div><p class="boundary-callout"><span aria-hidden="true">!</span><span><strong>最近有动静 ≠ 已完成</strong><br />文件发生变化只能说明有活动，不能代替正式进度记录。</span></p></section>
        <section class="panel-section" aria-labelledby="messages-title"><div class="section-heading"><div><p class="eyebrow">沟通记录</p><h2 id="messages-title">写入本机，不等于对方收到</h2></div><span class="heading-note">${localMode ? "本机事实" : "演示消息"}</span></div><div class="message-list">${renderMessageCard("local-written")}${renderMessageCard("delivery")}</div><div class="send-boundary"><span class="send-boundary__icon" aria-hidden="true">×</span><div><strong>当前不能发送真实消息</strong><p>没有真实连接和送达回执，按钮只提供查看依据或手动交接。</p></div></div></section>
      </div>
      <div class="progress-actions"><button class="button button--muted" type="button" disabled>${localMode ? "本机记录不会打开真实工具" : "演示不会打开真实工具"}</button><button class="button button--outline" type="button" data-action="replay-demo">${localMode ? "重新查看本地状态" : "重播演示状态"}</button><button class="button button--quiet" type="button" data-action="back-to-connect">连接真实工具</button></div>
    </section>`;
}

function renderTechnicalDetails(): string {
  const source = isLocalGroup()
    ? "Rust matrix-core 写入的本机 SQLite 事实源"
    : state.runtime.source === "browser-fallback"
      ? "浏览器内置演示回退"
      : "Rust offline_demo 命令返回";
  return `
    <details class="technical-details">
      <summary><span>查看连接与技术详情</span><small>默认折叠，不会改变当前状态</small></summary>
      <div class="technical-body">
        <p class="technical-lede">这里的信息用于排查连接问题。它不会改变协作组分工，也不会自动开放新的权限。</p>
        <dl class="technical-list">
          <div><dt>页面数据来源</dt><dd>${source}</dd></div>
          <div><dt>离线 fixture</dt><dd>${isLocalGroup() ? "local-group · 仅本机 SQLite · network=false" : "offline-demo · data_source=demo · network_required=false"}</dd></div>
          <div><dt>工具发现</dt><dd>${state.toolDiscovery ? `real · user_triggered · ${state.toolDiscovery.tools.filter((tool) => tool.status === "available").length}/${state.toolDiscovery.tools.length} 个入口可用` : "not_run · 不读取本机"}</dd></div>
          <div><dt>本地运行基线</dt><dd>${escapeHtml(state.runtime.doctorStatus)} · ${escapeHtml(state.runtime.doctorSummary)}</dd></div>
          <div><dt>观察时间</dt><dd>${escapeHtml(observedLabel())}</dd></div>
        </dl>
        <div class="technical-columns"><div><h3>已读取</h3><ul><li>离线演示结构</li><li>工作包和消息事件的状态字段</li><li>允许显示的脱敏说明</li></ul></div><div><h3>未读取 / 未执行</h3><ul><li>对话正文与凭据内容</li><li>真实工具探针与工作窗口创建</li><li>真实发送、恢复、取消和外部 Agent</li></ul></div></div>
        <p class="technical-foot">${escapeHtml(state.runtime.doctorDetail)}</p>
      </div>
    </details>`;
}

function renderEvidenceDrawer(): string {
  if (!state.evidence) return "";
  const record = evidenceRecords[state.evidence];
  return `
    <div class="drawer-layer" data-testid="evidence-drawer">
      <button class="drawer-backdrop" type="button" aria-label="关闭依据" data-action="close-evidence"></button>
      <section class="evidence-drawer" role="dialog" aria-modal="true" aria-labelledby="evidence-title">
        <div class="drawer-header"><div><p class="eyebrow">查看依据</p><h2 id="evidence-title">${escapeHtml(record.title)}</h2></div><button class="drawer-close" type="button" aria-label="关闭依据" data-action="close-evidence">×</button></div>
        <div class="drawer-status">${statusTag(record.status, state.evidence === "demo-boundary" ? "demo" : "unknown", state.evidence === "demo-boundary" ? "示" : "○")}</div>
        <dl class="evidence-list"><div><dt>依据</dt><dd>${escapeHtml(record.source)}</dd></div><div><dt>观察时间</dt><dd>${escapeHtml(record.observed)}</dd></div><div><dt>目前能确认</dt><dd>${escapeHtml(record.canConfirm)}</dd></div><div><dt>目前不能确认</dt><dd>${escapeHtml(record.cannotConfirm)}</dd></div><div><dt>下一步</dt><dd>${escapeHtml(record.next)}</dd></div></dl>
        <button class="button button--primary button--full" type="button" data-action="close-evidence">回到当前页面</button>
      </section>
    </div>`;
}

function render(): void {
  const screen = state.screen === 1 ? renderScreenOne() : state.screen === 2 ? renderScreenTwo() : renderScreenThree();
  app.innerHTML = `<div class="app-shell">${renderHeader()}<main class="content-column">${screen}</main>${renderTechnicalDetails()}<footer class="app-footer"><span>${isLocalGroup() ? "M1 本机事实源" : state.toolDiscovery ? "M2 本机能力快照" : "P1-A 离线演示边界"}</span><span>不创建 · 不发送 · 不读取正文</span></footer>${renderEvidenceDrawer()}</div>`;
  updateGroupButton();
  if (state.evidence) {
    requestAnimationFrame(() => app.querySelector<HTMLButtonElement>(".drawer-close")?.focus());
  }
}

function updateGroupButton(): void {
  const button = app.querySelector<HTMLButtonElement>('[data-action="build-group"]');
  if (button) button.disabled = state.goal.trim().length === 0 || state.runtime.demoStatus !== "ready" || state.creating;
  const count = app.querySelector<HTMLElement>("#goal-count");
  if (count) count.textContent = `${state.goal.length}/120`;
}

function enterDemo(): void {
  const group = currentGroup();
  if (!group || state.runtime.demoStatus !== "ready") return;
  state.screen = 2;
  state.created = false;
  state.creating = false;
  state.localGroup = null;
  state.idempotencyKey = globalThis.crypto?.randomUUID?.() ?? `group-${Date.now()}`;
  state.formError = null;
  if (!state.goal) state.goal = group.goal;
  state.evidence = null;
  render();
  requestAnimationFrame(() => app.querySelector<HTMLTextAreaElement>("#goal-input")?.focus());
}

async function buildGroup(): Promise<void> {
  if (!state.goal.trim()) {
    state.formError = "先写下你要完成的事，一句话就可以。";
    render();
    requestAnimationFrame(() => app.querySelector<HTMLTextAreaElement>("#goal-input")?.focus());
    return;
  }
  if (!state.runtime.demo) return;
  state.formError = null;
  if (hasTauri) {
    state.creating = true;
    render();
    try {
      const response = await invoke<CliResponse<CollaborationGroup>>("create_group", {
        goal: state.goal,
        templateId: state.template,
        idempotencyKey: state.idempotencyKey,
      });
      if (!response.ok || !response.data) {
        state.formError = response.errors[0]?.message ?? "本机事实源没有接受这个协作组";
        state.creating = false;
        render();
        return;
      }
      state.localGroup = response.data;
    } catch {
      state.formError = "暂时没能保存本机事实源，请稍后重试。";
      state.creating = false;
      render();
      return;
    }
  }
  state.creating = false;
  state.created = true;
  state.screen = 3;
  state.evidence = null;
  render();
}

async function discoverTools(): Promise<void> {
  if (!hasTauri || state.discovering) return;
  state.discoveryError = null;
  state.discovering = true;
  render();
  try {
    const response = await invoke<CliResponse<ToolDiscoverySnapshot>>("discover_tools");
    if (!response.ok || !response.data) {
      state.discoveryError = response.errors[0]?.message ?? "本机工具检查未完成。";
    } else {
      state.toolDiscovery = response.data;
    }
  } catch {
    state.discoveryError = "暂时没能读取本机工具入口；页面没有启动任何任务。";
  }
  state.discovering = false;
  render();
}

function closeEvidence(): void {
  state.evidence = null;
  render();
  if (lastFocusedElement?.isConnected) {
    lastFocusedElement.focus();
  }
  lastFocusedElement = null;
}

function openEvidence(id: EvidenceId): void {
  if (!evidenceRecords[id]) return;
  lastFocusedElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  state.evidence = id;
  render();
}

function goToStep(step: Screen): void {
  if (step === 3 && !state.created) return;
  if (step === 2 && state.runtime.demoStatus !== "ready") return;
  if (step === 2 && state.created) {
    state.created = false;
    state.localGroup = null;
    state.idempotencyKey = globalThis.crypto?.randomUUID?.() ?? `group-${Date.now()}`;
  }
  if (step === 2 && !state.idempotencyKey) {
    state.idempotencyKey = globalThis.crypto?.randomUUID?.() ?? `group-${Date.now()}`;
  }
  state.screen = step;
  state.evidence = null;
  render();
}

async function handleAction(element: HTMLElement): Promise<void> {
  const action = element.dataset.action;
  switch (action) {
    case "enter-demo":
      enterDemo();
      break;
    case "build-group":
      await buildGroup();
      break;
    case "discover-tools":
      await discoverTools();
      break;
    case "open-evidence":
      openEvidence((element.dataset.evidence ?? "demo-boundary") as EvidenceId);
      break;
    case "close-evidence":
      closeEvidence();
      break;
    case "back-to-connect":
      goToStep(1);
      break;
    case "replay-demo":
      state.screen = 3;
      state.evidence = null;
      render();
      break;
    case "go-step":
      goToStep(Number(element.dataset.step) as Screen);
      break;
    case "select-template":
      state.template = (element.dataset.template ?? "compact") as TemplateId;
      render();
      break;
    default:
      break;
  }
}

app.addEventListener("click", (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const actionElement = target.closest<HTMLElement>("[data-action]");
  if (actionElement) void handleAction(actionElement);
});

app.addEventListener("input", (event) => {
  const target = event.target;
  if (!(target instanceof HTMLTextAreaElement) || target.id !== "goal-input") return;
  state.goal = target.value.slice(0, 120);
  state.formError = null;
  updateGroupButton();
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && state.evidence) closeEvidence();
});

async function loadDesktopState(): Promise<void> {
  if (!hasTauri) return;

  try {
    const response = await invoke<CliResponse<DoctorData>>("doctor");
    const summary = response.data?.summary;
    state.runtime.doctorStatus = response.status;
    state.runtime.doctorObservedAt = response.data?.observed_at ?? null;
    state.runtime.doctorSummary = summary
      ? `必需检查通过 ${summary.pass} 项，未运行可选检查 ${summary.not_run} 项`
      : "doctor 未返回可显示的检查摘要";
    state.runtime.doctorDetail = response.ok
      ? "Rust doctor 已返回本地运行基线；它没有执行工具探针。"
      : "Rust doctor 返回了受限结果；页面不会把它解释成工具已连接。";
  } catch {
    state.runtime.doctorStatus = "读取受限";
    state.runtime.doctorSummary = "暂时没能读取本地运行基线";
    state.runtime.doctorDetail = "页面没有修改工具内容；请查看桌面壳运行状态。";
  }

  try {
    state.runtime.demo = await invoke<OfflineDemo>("offline_demo");
    state.runtime.demoStatus = "ready";
  } catch {
    state.runtime.demo = null;
    state.runtime.demoStatus = "blocked";
    state.runtime.doctorDetail = "随包离线 fixture 校验失败，页面没有生成替代数据。";
  }
  render();
}

render();
void loadDesktopState();
