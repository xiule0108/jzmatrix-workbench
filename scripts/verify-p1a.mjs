import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const checks = [];
const failures = [];

function record(id, ok, evidence, detail = null) {
  const result = { id, status: ok ? "pass" : "blocked", evidence, detail };
  checks.push(result);
  if (!ok) failures.push({ code: id, message: detail ?? "安全基线未满足" });
}

function readText(path) {
  return readFileSync(join(repoRoot, path), "utf8");
}

function readJson(path) {
  return JSON.parse(readText(path));
}

function walk(path) {
  const absolute = join(repoRoot, path);
  if (!existsSync(absolute)) return [];
  const entry = statSync(absolute);
  if (entry.isFile()) return [path];
  return readdirSync(absolute, { withFileTypes: true }).flatMap((child) =>
    walk(join(path, child.name)),
  );
}

function normalizedPath(path) {
  return path.replaceAll("\\", "/");
}

const rootPackage = readJson("package.json");
record(
  "toolchain.node_npm",
  rootPackage.packageManager === "npm@11.9.0" &&
    rootPackage.engines?.node === "24.14.0" &&
    rootPackage.engines?.npm === "11.9.0",
  "package.json:packageManager+engines",
  "Node.js 24.14.0 and npm 11.9.0 are required",
);

const toolchain = readText("rust-toolchain.toml");
const rustManifests = [
  "apps/desktop/src-tauri/Cargo.toml",
  "crates/jzmatrix-cli/Cargo.toml",
  "crates/matrix-core/Cargo.toml",
];
const bootstrap = readText("scripts/bootstrap.sh");
const verifyWorkflow = readText(".github/workflows/verify.yml");
record(
  "toolchain.rust",
  /channel\s*=\s*"1\.88\.0"/.test(toolchain) &&
    /profile\s*=\s*"minimal"/.test(toolchain) &&
    /rustfmt/.test(toolchain) &&
    /clippy/.test(toolchain) &&
    rustManifests.every((path) => /rust-version\s*=\s*"1\.88\.0"/.test(readText(path))) &&
    /rust_version="1\.88\.0"/.test(bootstrap) &&
    /rustup toolchain install 1\.88\.0/.test(verifyWorkflow),
  "rust-toolchain.toml+workspace Cargo.toml files+bootstrap+verify workflow",
  "Rust 1.88.0 must be synchronized across the minimal toolchain, workspace crates, bootstrap, and CI",
);

const cargoLockPath = join(repoRoot, "Cargo.lock");
const npmLockPath = join(repoRoot, "package-lock.json");
record(
  "locks.present",
  existsSync(cargoLockPath) && existsSync(npmLockPath),
  "Cargo.lock+package-lock.json",
  "Both dependency lockfiles must be committed",
);
if (existsSync(cargoLockPath)) {
  const cargoLock = readText("Cargo.lock");
  record("locks.cargo_version", /version = 4/.test(cargoLock), "Cargo.lock:version");
  const cargoPackages = cargoLock
    .split("[[package]]")
    .slice(1)
    .map((block) => ({
      name: /^name = "([^"]+)"$/m.exec(block)?.[1] ?? null,
      version: /^version = "([^"]+)"$/m.exec(block)?.[1] ?? null,
    }));
  const hasCargoPackage = (name, version) =>
    cargoPackages.some((entry) => entry.name === name && entry.version === version);
  record(
    "locks.cargo_security_graph",
    hasCargoPackage("tauri", "2.11.1") &&
      hasCargoPackage("tauri-build", "2.6.1") &&
      hasCargoPackage("serde_with", "3.21.0") &&
      hasCargoPackage("time", "0.3.47") &&
      !hasCargoPackage("rand", "0.7.3") &&
      hasCargoPackage("glib", "0.18.5"),
    "Cargo.lock:Tauri/serde_with/time/rand/glib",
    "The reviewed Tauri security graph must remain locked and the Linux/BSD glib blocker must stay visible",
  );
}
if (existsSync(npmLockPath)) {
  const npmLock = readJson("package-lock.json");
  record(
    "locks.npm_version",
    npmLock.lockfileVersion === 3 && npmLock.packages?.[""],
    "package-lock.json:lockfileVersion",
    "npm lockfileVersion 3 is required",
  );
  const desktopLock = npmLock.packages?.["apps/desktop"] ?? {};
  record(
    "locks.npm_tauri_gate",
    desktopLock.dependencies?.["@tauri-apps/api"] === "=2.11.1" &&
      desktopLock.devDependencies?.["@tauri-apps/cli"] === "=2.11.1" &&
      desktopLock.devDependencies?.vite === "=6.1.0" &&
      npmLock.packages?.["node_modules/@tauri-apps/api"]?.version === "2.11.1" &&
      npmLock.packages?.["node_modules/@tauri-apps/cli"]?.version === "2.11.1" &&
      npmLock.packages?.["node_modules/vite"]?.version === "6.1.0",
    "package-lock.json:desktop Tauri API/CLI and isolated Vite gate",
    "Tauri API/CLI must be 2.11.1 while Vite remains exactly 6.1.0 in this gate",
  );
}

const fixtureManifest = readJson("fixtures/offline-demo/manifest.json");
const fixtureBytes = readFileSync(join(repoRoot, "fixtures/offline-demo", fixtureManifest.fixture));
const fixtureHash = createHash("sha256").update(fixtureBytes).digest("hex");
record(
  "fixture.integrity",
  fixtureManifest.contract === "jzmatrix.offline-demo-manifest" &&
    fixtureManifest.version === "1.0.0" &&
    fixtureManifest.data_source === "demo" &&
    fixtureManifest.network_required === false &&
    fixtureHash === fixtureManifest.sha256,
  `fixtures/offline-demo/${fixtureManifest.fixture}:sha256`,
  "Offline fixture must be fixed, demo-only, and network-free",
);

const platformManifest = readJson("fixtures/platform-events/manifest.json");
const platformFixtureEntries = platformManifest.fixtures ?? [];
const platformFixtureIds = platformFixtureEntries.map((entry) => entry.id).sort();
const platformFixtureText = platformFixtureEntries
  .filter(
    (entry) =>
      typeof entry.file === "string" &&
      !entry.file.includes("/") &&
      !entry.file.includes("\\") &&
      !entry.file.includes(".."),
  )
  .map((entry) => readText(`fixtures/platform-events/${entry.file}`))
  .join("\n");
const platformFixtureForbidden =
  /(?:\/Users\/|\/home\/|[A-Za-z]:\\|\\\\|https?:\/\/|file:\/\/|api[_-]?key|access[_-]?token|refresh[_-]?token|authorization|password|private[_-]?key|client[_-]?secret|transcript|prompt|response|tool[_-]?(?:input|output))/i;
const platformFixtureHashesMatch = platformFixtureEntries.every((entry) => {
  if (
    typeof entry.file !== "string" ||
    entry.file.includes("/") ||
    entry.file.includes("\\") ||
    entry.file.includes("..")
  ) {
    return false;
  }
  const bytes = readFileSync(join(repoRoot, "fixtures/platform-events", entry.file));
  return createHash("sha256").update(bytes).digest("hex") === entry.sha256;
});
record(
  "fixture.platform_events_integrity",
  platformManifest.contract === "jzmatrix.synthetic-platform-fixture-manifest" &&
    platformManifest.version === "1.0.0" &&
    platformManifest.parser_version === "1.0.0" &&
    platformManifest.synthetic === true &&
    platformManifest.network_required === false &&
    platformManifest.source_observation === "10A_P0_platform_probe" &&
    platformManifest.protocol_coverage === "observed_structured_event_categories_only" &&
    platformFixtureIds.join(",") === "claude-code-synthetic-v1,codex-cli-synthetic-v1" &&
    platformFixtureEntries.every(
      (entry) =>
        entry.platform &&
        entry.sha256 &&
        Array.isArray(entry.observed_event_kinds) &&
        entry.observed_event_kinds.length > 0,
    ) &&
    platformFixtureHashesMatch &&
    !platformFixtureForbidden.test(platformFixtureText),
  "fixtures/platform-events/manifest.json+fixture sha256",
  "Synthetic platform fixtures must be embedded, fixed, safe, and network-free",
);

const productRuntimeFiles = [
  ...walk("crates").filter(
    (path) => path.endsWith(".rs") && !normalizedPath(path).includes("/tests/"),
  ),
  ...walk("apps/desktop/src").filter((path) => path.endsWith(".ts") || path.endsWith(".css")),
  ...walk("apps/desktop/src-tauri").filter(
    (path) =>
      !normalizedPath(path).startsWith("apps/desktop/src-tauri/gen/") &&
      (path.endsWith(".rs") || path.endsWith(".json")),
  ),
];
const developmentShellScriptFiles = walk("scripts").filter((path) => path.endsWith(".sh"));
const maintenanceScriptFiles = walk("scripts").filter(
  (path) => path.endsWith(".mjs") && normalizedPath(path) !== "scripts/verify-p1a.mjs",
);
const reviewedSourceFiles = [
  ...productRuntimeFiles,
  ...developmentShellScriptFiles,
  ...maintenanceScriptFiles,
];
const forbiddenLocalReference = /(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\|\.codex|\.claude|\.agents|\.ssh|\.aws)/i;
const localReferenceHits = reviewedSourceFiles.flatMap((path) => {
  const text = readText(path);
  return forbiddenLocalReference.test(text) ? [path] : [];
});
record(
  "security.no_user_paths",
  localReferenceHits.length === 0,
  "implementation source tree",
  localReferenceHits.length === 0 ? null : `forbidden local references: ${localReferenceHits.join(", ")}`,
);

const productRuntimeText = productRuntimeFiles
  .filter(
    (path) =>
      !path.endsWith("tauri.conf.json") &&
      normalizedPath(path) !== "crates/matrix-core/src/tool_discovery.rs",
  )
  .map((path) => readText(path))
  .join("\n");
const runtimeNetworkOrProcessPattern =
  /\b(?:fetch|XMLHttpRequest|WebSocket)\s*\(|\b(?:reqwest|ureq|hyper)::|\b(?:std|tokio)::net::|\b(?:std|tokio)::process::Command\b|\bCommand::new\s*\(|\b(?:child_process|node:child_process)\b|\b(?:Bun\.spawn|Deno\.Command)\b/;
record(
  "security.no_runtime_network_or_shell",
  !runtimeNetworkOrProcessPattern.test(productRuntimeText),
  "product runtime only: Rust core/CLI, Tauri runtime, and frontend",
  "P1-A product runtime must not open network connections or start processes",
);

const toolDiscoveryText = readText("crates/matrix-core/src/tool_discovery.rs");
const allowedDiscoveryBinaries = [
  "codex",
  "claude",
  "cursor-agent",
  "copilot",
  "zed",
  "zcode",
  "code",
  "opencode",
  "cline",
  "aider",
];
record(
  "security.tool_discovery_allowlist",
  toolDiscoveryText.includes("const ALLOWED_TOOL_SPECS") &&
    allowedDiscoveryBinaries.every((binary) => toolDiscoveryText.includes(`binary: "${binary}"`)) &&
    toolDiscoveryText.includes('run_probe(&path, "--version")') &&
    toolDiscoveryText.includes('run_probe(&path, "--help")') &&
    !/Command::new\(\s*"(?:sh|bash|zsh|fish|cmd|powershell|pwsh)"\s*\)/.test(toolDiscoveryText) &&
    !/\b(?:std::env::args|std::env::args_os)\s*\(/.test(toolDiscoveryText),
  "crates/matrix-core/src/tool_discovery.rs:fixed binary and argument allowlist",
  "M2 permits only user-triggered --version/--help probes for ten named binaries",
);

const dryRunText = readText("crates/matrix-core/src/dry_run.rs");
record(
  "security.dry_run_no_execution",
  dryRunText.includes('execution: "not_authorized"') &&
    ["create", "send", "resume", "cancel"].every((operation) =>
      dryRunText.includes(`id: "${operation}"`),
    ) &&
    !/\b(?:std::process::Command|tokio::process::Command|Command::new\s*\()/.test(dryRunText),
  "crates/matrix-core/src/dry_run.rs:plan-only execution boundary",
  "M4 plans create/send/resume/cancel but cannot start processes or authorize external writes",
);

const allowedShellCommands = new Map([
  [
    "scripts/bootstrap.sh",
    new Set([
      "curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \"$rustup_url\" --output \"$temp_dir/rustup-init.sh\"",
      "sh \"$temp_dir/rustup-init.sh\" -y --profile minimal --default-toolchain \"$rust_version\"",
    ]),
  ],
  [
    "scripts/doctor.sh",
    new Set(["exec cargo run --offline --locked -p jzmatrix-cli -- doctor --json"]),
  ],
]);
const riskyShellCommandPattern =
  /(?:^|[;&|(){}]\s*)\s*(?:(?:if|then|do|else|elif|while|until|!)\s+)*(?:(?:[A-Za-z_][A-Za-z0-9_]*=(?:"[^"]*"|'[^']*'|[^\s]+))\s+)*(?:(?:command|builtin|sudo|env)\s+)*(?:[^\s;&|(){}]+\/)?(curl|wget|nc|ncat|ssh|scp|sh|bash|eval|exec)(?=\s|$)/;
const shellRiskHits = developmentShellScriptFiles.flatMap((path) => {
  const allowlist = allowedShellCommands.get(normalizedPath(path)) ?? new Set();
  return readText(path)
    .split(/\r?\n/u)
    .flatMap((line, index) => {
      const trimmed = line.trim();
      const match = riskyShellCommandPattern.exec(line);
      if (!match || trimmed.startsWith("#") || allowlist.has(trimmed)) return [];
      return [`${normalizedPath(path)}:${index + 1}:${match[1]}`];
    });
});
record(
  "security.dev_scripts_no_unapproved_network_or_shell",
  shellRiskHits.length === 0,
  "development shell scripts with path-and-command allowlist",
  shellRiskHits.length === 0
    ? null
    : `unapproved network or command startup: ${shellRiskHits.join(", ")}`,
);

const tauriConfig = readJson("apps/desktop/src-tauri/tauri.conf.json");
const csp = tauriConfig.app?.security?.csp ?? "";
record(
  "security.tauri_defaults",
  tauriConfig.app?.withGlobalTauri === false &&
    tauriConfig.bundle?.createUpdaterArtifacts === false &&
    tauriConfig.build?.devUrl === "http://localhost:1420" &&
    csp.includes("default-src 'self'") &&
    csp.includes("connect-src 'self' ipc: http://ipc.localhost"),
  "apps/desktop/src-tauri/tauri.conf.json:security/bundle/build",
  "Tauri must load local resources and keep updater artifacts disabled",
);

const capabilities = readJson("apps/desktop/src-tauri/capabilities/main.json");
const permissions = capabilities.permissions ?? [];
record(
  "security.capabilities_minimum",
  permissions.length === 1 && permissions[0] === "core:default" &&
    !permissions.some((permission) => /(?:shell|sql|updater|http|fs:all|process)/i.test(permission)),
  "apps/desktop/src-tauri/capabilities/main.json:permissions",
  "Only the typed core command surface is permitted in P1-A",
);

const tauriCommands = readText("apps/desktop/src-tauri/src/lib.rs").match(/#\[tauri::command\]/g) ?? [];
const tauriCommandSurface = readText("apps/desktop/src-tauri/src/lib.rs").replace(/\s/g, "");
record(
  "security.typed_command_surface",
  tauriCommands.length === 6 &&
    tauriCommandSurface.includes(
      "generate_handler![doctor,offline_demo,group_templates,create_group,discover_tools,plan_preview]",
    ),
  "apps/desktop/src-tauri/src/lib.rs:commands",
  "M4 adds only the typed local plan_preview command; no generic process or network command is exposed",
);

const windowsOriginProbe = readText("scripts/verify-windows-origin-confusion.mjs");
record(
  "security.windows_origin_confusion_probe",
  /tauri\.evil\.test/.test(windowsOriginProbe) &&
    /__TAURI_INTERNALS__/.test(windowsOriginProbe) &&
    /"doctor", "offline_demo"/.test(windowsOriginProbe) &&
    /app_process_alive/.test(windowsOriginProbe) &&
    /node scripts\/verify-windows-origin-confusion\.mjs/.test(verifyWorkflow) &&
    /AdditionalBrowserArguments/.test(verifyWorkflow) &&
    /windows-webview2-diagnostics\.json/.test(verifyWorkflow) &&
    /browser_args_value_restored/.test(verifyWorkflow) &&
    /hosts_marker_residual/.test(verifyWorkflow) &&
    /if: always\(\) && runner\.os == 'Windows'/.test(verifyWorkflow),
  "Windows WebView2 remote-origin probe+verify workflow",
  "Windows CI must load a non-local tauri.* origin in WebView2 and prove both typed commands are blocked",
);

const result = {
  contract: "jzmatrix.p1a.verify",
  version: "1.0.0",
  ok: failures.length === 0,
  status: failures.length === 0 ? "pass" : "blocked",
  checks,
  errors: failures,
  boundary: {
    offline_by_default: true,
    real_adapters: false,
    external_agent_writes: false,
    external_processes: "allowlisted_user_triggered_only",
    windows_support_claim: false,
  },
};
console.log(JSON.stringify(result, null, 2));
process.exitCode = failures.length === 0 ? 0 : 1;
