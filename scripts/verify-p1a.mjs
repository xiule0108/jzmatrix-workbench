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
  record("locks.cargo_version", /version = 4/.test(readText("Cargo.lock")), "Cargo.lock:version");
}
if (existsSync(npmLockPath)) {
  const npmLock = readJson("package-lock.json");
  record(
    "locks.npm_version",
    npmLock.lockfileVersion === 3 && npmLock.packages?.[""],
    "package-lock.json:lockfileVersion",
    "npm lockfileVersion 3 is required",
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
  .filter((path) => !path.endsWith("tauri.conf.json"))
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
record(
  "security.typed_command_surface",
  tauriCommands.length === 2 &&
    /generate_handler!\[doctor, offline_demo\]/.test(readText("apps/desktop/src-tauri/src/lib.rs")),
  "apps/desktop/src-tauri/src/lib.rs:commands",
  "P1-A exposes only doctor and offline_demo",
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
    windows_support_claim: false,
  },
};
console.log(JSON.stringify(result, null, 2));
process.exitCode = failures.length === 0 ? 0 : 1;
