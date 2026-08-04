import childProcess, { spawnSync } from "node:child_process";
import fs, {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { connect } from "node:net";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const originalExistsSync = fs.existsSync;
const originalExec = childProcess.exec;
const originalSpawn = childProcess.spawn;
const launchSideEffectProbe = {
  active: false,
  filesystemPaths: [],
  childProcessMethods: [],
};

fs.existsSync = (candidate) => {
  if (launchSideEffectProbe.active) {
    const candidatePath = String(candidate);
    launchSideEffectProbe.filesystemPaths.push(candidatePath);
    if (candidatePath.startsWith("\\\\")) {
      throw new Error("test harness blocked an unexpected UNC filesystem probe");
    }
  }
  return originalExistsSync(candidate);
};
childProcess.exec = (...values) => {
  if (launchSideEffectProbe.active) {
    launchSideEffectProbe.childProcessMethods.push("exec");
    throw new Error(`test harness blocked an unexpected exec: ${String(values[0])}`);
  }
  return originalExec(...values);
};
childProcess.spawn = (...values) => {
  if (launchSideEffectProbe.active) {
    launchSideEffectProbe.childProcessMethods.push("spawn");
    throw new Error(`test harness blocked an unexpected spawn: ${String(values[0])}`);
  }
  return originalSpawn(...values);
};

// Install the side-effect interceptors before Vite loads so its bundled launch-editor
// observes the same fs and child_process functions even if it captures them at import time.
const { createLogger, createServer } = await import("vite");

const CONTRACT = "jzmatrix.vite-security-verification";
const VERSION = "1.0.0";
const HOST = "127.0.0.1";
const isWindows = process.platform === "win32";
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const temporaryRoot = mkdtempSync(join(tmpdir(), "jzmatrix-vite-security-"));
const projectRoot = join(temporaryRoot, "project-with-long-security-name");
const publicRoot = join(projectRoot, "p");
const outsideTextPath = join(temporaryRoot, "outside-canary.txt");
const outsideHtmlPath = join(temporaryRoot, "outside-canary.html");
const outsideMapPath = join(temporaryRoot, "outside-canary.map");
const privatePath = join(projectRoot, ".env.security-private.txt");
const deniedLongPath = join(projectRoot, ".env.security-canary-configuration");
const safeMarker = "JZMATRIX_SAFE_CONTROL_64C3";
const secretMarker = "JZMATRIX_DENIED_CANARY_64C3";
const cases = [];

const advisoryCoverage = {
  "GHSA-x574-m823-4x7w": "raw query separators",
  "GHSA-4r4m-qw57-chr8": "import with raw or inline",
  "GHSA-xcj6-pq6g-qj4x": "svg and relative paths",
  "GHSA-356w-63v5-8wf4": "invalid request-target",
  "GHSA-859w-5945-r5v3": "slash-dot deny bypass",
  "GHSA-g4jq-h2w9-997c": "public directory prefix traversal",
  "GHSA-jqfw-vq24-v9c3": "HTML outside root",
  "GHSA-93m4-6634-74q7": "Windows backslash deny bypass",
  "GHSA-p9ff-h696-f583": "WebSocket fetchModule arbitrary read",
  "GHSA-4w7w-66w2-5vf9": "optimized dependencies source-map traversal",
  "GHSA-fx2h-pf6j-xcff": "Windows ADS and 8.3 alternate paths",
  "GHSA-v6wh-96g9-6wx3": "Windows UNC launch-editor NTLM disclosure",
};

function addResult(id, status, evidence = {}, advisories = []) {
  cases.push({ id, status, advisories, evidence });
}

async function runCase(id, fn, { windowsOnly = false, advisories = [] } = {}) {
  if (windowsOnly && !isWindows) {
    addResult(id, "skipped", { reason: "windows_only" }, advisories);
    return;
  }
  try {
    addResult(id, "pass", await fn(), advisories);
  } catch (error) {
    addResult(
      id,
      "failed",
      { error: error instanceof Error ? error.message : String(error) },
      advisories,
    );
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function parseVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)/u.exec(version);
  if (!match) throw new Error(`invalid dependency version: ${version}`);
  return match.slice(1).map(Number);
}

function versionAtLeast(actual, minimum) {
  const left = parseVersion(actual);
  const right = parseVersion(minimum);
  for (let index = 0; index < 3; index += 1) {
    if (left[index] !== right[index]) return left[index] > right[index];
  }
  return true;
}

function parseRawResponse(raw) {
  const splitAt = raw.indexOf("\r\n\r\n");
  const head = splitAt === -1 ? raw : raw.slice(0, splitAt);
  const body = splitAt === -1 ? "" : raw.slice(splitAt + 4);
  const match = /^HTTP\/1\.[01]\s+(\d{3})/u.exec(head);
  if (!match) throw new Error("loopback server returned an invalid HTTP response");
  return { status: Number(match[1]), body };
}

function rawRequest(port, target) {
  return new Promise((resolveRequest, rejectRequest) => {
    const chunks = [];
    const socket = connect({ host: HOST, port });
    socket.setTimeout(10_000);
    socket.on("connect", () => {
      socket.write(
        `GET ${target} HTTP/1.1\r\nHost: ${HOST}:${port}\r\nAccept: */*\r\nConnection: close\r\n\r\n`,
      );
    });
    socket.on("data", (chunk) => chunks.push(chunk));
    socket.on("end", () => resolveRequest(parseRawResponse(Buffer.concat(chunks).toString("utf8"))));
    socket.on("timeout", () => socket.destroy(new Error("loopback HTTP request timed out")));
    socket.on("error", rejectRequest);
  });
}

function assertDenied(response, id, acceptedStatuses = [403, 404]) {
  assert(acceptedStatuses.includes(response.status), `${id} returned HTTP ${response.status}`);
  assert(!response.body.includes(secretMarker), `${id} exposed the denied canary`);
  return { http_status: response.status, denied_canary_absent: true };
}

function normalizedFsPath(path) {
  return path.replaceAll("\\", "/");
}

function fsRequestPath(path) {
  const normalized = normalizedFsPath(path);
  return `/@fs/${normalized}`;
}

function getPackageVersion(name) {
  const result = spawnSync(process.execPath, ["-p", `require('${name}/package.json').version`], {
    encoding: "utf8",
    cwd: repoRoot,
  });
  if (result.status !== 0) throw new Error(`cannot resolve ${name} version`);
  return result.stdout.trim();
}

function getWindowsFileSystem(path) {
  const drive = /^([A-Za-z]):/u.exec(resolve(path))?.[1];
  if (!drive) throw new Error("temporary directory is not on a Windows drive");
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", `(Get-Volume -DriveLetter '${drive}').FileSystem`],
    { encoding: "utf8" },
  );
  if (result.status !== 0) throw new Error("cannot identify the Windows temporary volume");
  return result.stdout.trim();
}

function getWindowsShortBasename(path) {
  const escaped = resolve(path).replaceAll("'", "''");
  const readShortName = () => {
    const result = spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        `(New-Object -ComObject Scripting.FileSystemObject).GetFile('${escaped}').ShortPath`,
      ],
      { encoding: "utf8" },
    );
    if (result.status !== 0) throw new Error("cannot obtain an NTFS 8.3 short name");
    return basename(result.stdout.trim());
  };

  let shortName = readShortName();
  let assignedForProbe = false;
  if (!shortName.includes("~")) {
    const assignedName = "ENVSEC~1.CFG";
    const assignment = spawnSync(
      "fsutil.exe",
      ["file", "setshortname", resolve(path), assignedName],
      { encoding: "utf8" },
    );
    if (assignment.status !== 0) {
      throw new Error("NTFS 8.3 short-name generation is unavailable and probe alias assignment failed");
    }
    shortName = readShortName();
    assignedForProbe = true;
  }
  if (!shortName.includes("~")) throw new Error("NTFS did not expose the 8.3 probe alias");
  return { shortName, assignedForProbe };
}

function inspectInstalledUncGuard() {
  const viteEntry = fileURLToPath(import.meta.resolve("vite"));
  const chunksDirectory = join(dirname(viteEntry), "chunks");
  const guardText = "UNC paths are not supported on Windows to avoid security issues.";
  for (const fileName of readdirSync(chunksDirectory)) {
    if (!fileName.endsWith(".js")) continue;
    const source = readFileSync(join(chunksDirectory, fileName), "utf8");
    const guardIndex = source.indexOf(guardText);
    if (guardIndex === -1) continue;
    const existsIndex = source.indexOf("existsSync(fileName)", guardIndex);
    const execIndex = source.indexOf("childProcess.exec", guardIndex);
    assert(existsIndex > guardIndex, "UNC guard does not precede the filesystem probe");
    assert(execIndex > existsIndex, "UNC guard does not precede the editor child process");
    return {
      guard_text_present: true,
      guard_precedes_filesystem_probe: true,
      guard_precedes_child_process: true,
    };
  }
  throw new Error("installed Vite bundle does not contain the Windows UNC guard");
}

function websocketInvoke(url, token, data) {
  return new Promise((resolveInvoke, rejectInvoke) => {
    const socket = new WebSocket(`${url}?token=${encodeURIComponent(token)}`, "vite-hmr");
    const timeout = setTimeout(() => {
      socket.close();
      rejectInvoke(new Error("Vite WebSocket invoke timed out"));
    }, 10_000);
    socket.addEventListener("open", () => {
      socket.send(
        JSON.stringify({
          type: "custom",
          event: "vite:invoke",
          data: { name: "fetchModule", id: "send:1", data },
        }),
      );
    });
    socket.addEventListener("message", (event) => {
      const parsed = JSON.parse(String(event.data));
      if (
        parsed.type === "custom" &&
        parsed.event === "vite:invoke" &&
        parsed.data?.id === "response:1"
      ) {
        clearTimeout(timeout);
        socket.close();
        resolveInvoke(parsed.data.data);
      }
    });
    socket.addEventListener("error", () => {
      clearTimeout(timeout);
      rejectInvoke(new Error("Vite WebSocket connection failed"));
    });
  });
}

mkdirSync(projectRoot, { recursive: true });
mkdirSync(publicRoot, { recursive: true });
mkdirSync(join(publicRoot, "target"), { recursive: true });
mkdirSync(join(projectRoot, "node_modules", ".vite", "deps"), { recursive: true });
writeFileSync(join(projectRoot, "package.json"), '{"name":"vite-security-probe","private":true}');
writeFileSync(join(projectRoot, "index.html"), `<main>${safeMarker}</main>`);
writeFileSync(join(projectRoot, "safe.txt"), safeMarker);
writeFileSync(join(projectRoot, ".env"), secretMarker);
writeFileSync(deniedLongPath, secretMarker);
writeFileSync(privatePath, secretMarker);
writeFileSync(outsideTextPath, secretMarker);
writeFileSync(outsideHtmlPath, `<main>${secretMarker}</main>`);
writeFileSync(
  outsideMapPath,
  JSON.stringify({ version: 3, file: "x.js", sources: [secretMarker], names: [], mappings: "" }),
);
writeFileSync(
  join(projectRoot, "node_modules", ".vite", "deps", "safe.js.map"),
  JSON.stringify({ version: 3, file: "safe.js", sources: [safeMarker], names: [], mappings: "" }),
);
symlinkSync(join(publicRoot, "target"), join(publicRoot, "link"), isWindows ? "junction" : "dir");

const logger = createLogger("silent", { allowClearScreen: false });
const server = await createServer({
  configFile: false,
  root: projectRoot,
  publicDir: publicRoot,
  customLogger: logger,
  server: {
    host: HOST,
    port: 0,
    strictPort: false,
    allowedHosts: [HOST],
    fs: {
      strict: true,
      allow: [projectRoot],
      deny: [".env", ".env.*", "*.{crt,pem}"],
    },
  },
});

try {
  await server.listen();
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not bind a TCP port");
  assert(address.address === HOST, `Vite bound unexpected host ${address.address}`);
  const port = address.port;
  const wsUrl = `ws://${HOST}:${port}/`;

  await runCase("dependency.vite_exact", async () => {
    const actual = getPackageVersion("vite");
    assert(actual === "6.4.3", `expected Vite 6.4.3, found ${actual}`);
    return { actual, expected: "6.4.3" };
  });

  await runCase("dependency.esbuild_floor", async () => {
    const actual = getPackageVersion("esbuild");
    assert(versionAtLeast(actual, "0.25.0"), `expected esbuild >=0.25.0, found ${actual}`);
    return { actual, minimum: "0.25.0", advisory: "GHSA-67mh-4wv8-2f99" };
  });

  await runCase("control.safe_resource", async () => {
    const response = await rawRequest(port, "/");
    assert(
      response.status === 200,
      `safe control returned HTTP ${response.status}; body=${JSON.stringify(response.body.slice(0, 500))}`,
    );
    assert(response.body.includes(safeMarker), "safe control marker was not returned");
    return { http_status: response.status, safe_marker_present: true };
  });

  await runCase("control.default_deny", async () => {
    return assertDenied(await rawRequest(port, "/.env?raw"), "default deny");
  });

  await runCase(
    "vite.raw_query_separators_denied",
    async () => assertDenied(await rawRequest(port, "/.env?raw??"), "raw query separators"),
    { advisories: ["GHSA-x574-m823-4x7w"] },
  );

  await runCase(
    "vite.import_inline_denied",
    async () =>
      assertDenied(
        await rawRequest(port, "/.env?import&?inline=1.wasm?init"),
        "import inline",
      ),
    { advisories: ["GHSA-4r4m-qw57-chr8"] },
  );

  await runCase(
    "vite.svg_query_denied",
    async () =>
      assertDenied(await rawRequest(port, "/.env?.svg?.wasm?init"), "svg query bypass"),
    { advisories: ["GHSA-xcj6-pq6g-qj4x"] },
  );

  await runCase(
    "vite.invalid_request_target_denied",
    async () => {
      const rootPath = normalizedFsPath(projectRoot);
      const target = `/@fs/${rootPath}/#/../../../../${basename(outsideTextPath)}`;
      return assertDenied(await rawRequest(port, target), "invalid request-target", [400, 403]);
    },
    { advisories: ["GHSA-356w-63v5-8wf4"] },
  );

  await runCase(
    "vite.slash_dot_denied",
    async () => assertDenied(await rawRequest(port, "/.env/."), "slash-dot deny bypass"),
    { advisories: ["GHSA-859w-5945-r5v3"] },
  );

  await runCase(
    "vite.outside_html_denied",
    async () =>
      assertDenied(
        await rawRequest(port, `/../${basename(outsideHtmlPath)}`),
        "outside HTML",
      ),
    { advisories: ["GHSA-jqfw-vq24-v9c3"] },
  );

  await runCase(
    "vite.public_prefix_traversal_denied",
    async () => {
      const normal = await rawRequest(port, `/${basename(privatePath)}`);
      assertDenied(normal, "public-prefix normal deny");
      const traversal = await rawRequest(port, `/../${basename(privatePath)}`);
      assert(!traversal.body.includes(secretMarker), "public-prefix traversal exposed the canary");
      assert(
        traversal.status === 403 ||
          (traversal.status === 200 && traversal.body.includes(safeMarker)),
        `public-prefix traversal was neither denied nor safely handled (HTTP ${traversal.status})`,
      );
      return {
        normal_http_status: normal.status,
        traversal_http_status: traversal.status,
        disposition: traversal.status === 403 ? "denied" : "safe_spa_fallback",
        denied_canary_absent: true,
      };
    },
    { advisories: ["GHSA-g4jq-h2w9-997c"] },
  );

  await runCase(
    "vite.websocket_fetch_module_denied",
    async () => {
      const result = await websocketInvoke(wsUrl, server.config.webSocketToken, [
        `${pathToFileURL(outsideTextPath).href}?raw`,
      ]);
      const serialized = JSON.stringify(result);
      assert(!serialized.includes(secretMarker), "WebSocket fetchModule exposed the canary");
      assert(result?.result === undefined && result?.error, "WebSocket fetchModule was not rejected");
      return { websocket_connected: true, invoke_rejected: true, denied_canary_absent: true };
    },
    { advisories: ["GHSA-p9ff-h696-f583"] },
  );

  await runCase(
    "vite.optimized_map_parent_traversal_denied",
    async () => {
      const target = `${fsRequestPath(projectRoot)}/node_modules/.vite/deps/../../../../${basename(outsideMapPath)}`;
      return assertDenied(await rawRequest(port, target), "optimized map parent traversal");
    },
    { advisories: ["GHSA-4w7w-66w2-5vf9"] },
  );

  await runCase(
    "windows.ntfs_volume_control",
    async () => {
      const fileSystem = getWindowsFileSystem(projectRoot);
      assert(fileSystem.toUpperCase() === "NTFS", `expected NTFS, found ${fileSystem}`);
      return { file_system: fileSystem.toUpperCase() };
    },
    { windowsOnly: true },
  );

  await runCase(
    "windows.ads_denied",
    async () =>
      assertDenied(await rawRequest(port, "/.env::$DATA?raw"), "NTFS ADS", [403, 404]),
    { windowsOnly: true, advisories: ["GHSA-fx2h-pf6j-xcff"] },
  );

  await runCase(
    "windows.short_name_denied",
    async () => {
      const { shortName, assignedForProbe } = getWindowsShortBasename(deniedLongPath);
      const evidence = assertDenied(
        await rawRequest(port, `/${shortName}?raw`),
        "NTFS 8.3 short name",
        [403],
      );
      return { ...evidence, short_name_has_tilde: true, assigned_for_probe: assignedForProbe };
    },
    { windowsOnly: true, advisories: ["GHSA-fx2h-pf6j-xcff"] },
  );

  await runCase(
    "windows.backslash_denied",
    async () => assertDenied(await rawRequest(port, "/.env\\"), "Windows backslash", [403]),
    { windowsOnly: true, advisories: ["GHSA-93m4-6634-74q7"] },
  );

  await runCase(
    "windows.optimized_map_backslash_denied",
    async () => {
      const target = `${fsRequestPath(projectRoot)}/node_modules/.vite/deps/..\\..\\..\\..\\${basename(outsideMapPath)}`;
      return assertDenied(await rawRequest(port, target), "optimized map backslash traversal", [403]);
    },
    { windowsOnly: true, advisories: ["GHSA-4w7w-66w2-5vf9"] },
  );

  await runCase(
    "windows.unc_launch_editor_denied",
    async () => {
      const sourceGuard = inspectInstalledUncGuard();
      const previousLaunchEditor = process.env.LAUNCH_EDITOR;
      process.env.LAUNCH_EDITOR = "jzmatrix-security-probe-editor";
      launchSideEffectProbe.active = true;
      launchSideEffectProbe.filesystemPaths = [];
      launchSideEffectProbe.childProcessMethods = [];
      let response;
      let controlFilesystemObserved = false;
      let controlChildProcessObserved = false;
      try {
        await rawRequest(
          port,
          `/__open-in-editor?file=${encodeURIComponent(join(projectRoot, "safe.txt"))}`,
        );
        controlFilesystemObserved = launchSideEffectProbe.filesystemPaths.some(
          (candidate) => resolve(candidate) === resolve(join(projectRoot, "safe.txt")),
        );
        controlChildProcessObserved = launchSideEffectProbe.childProcessMethods.length === 1;
        launchSideEffectProbe.filesystemPaths = [];
        launchSideEffectProbe.childProcessMethods = [];

        const uncPath = "\\\\127.0.0.1\\jzmatrix-no-share\\canary.txt";
        response = await rawRequest(
          port,
          `/__open-in-editor?file=${encodeURIComponent(uncPath)}`,
        );
      } finally {
        launchSideEffectProbe.active = false;
        if (previousLaunchEditor === undefined) delete process.env.LAUNCH_EDITOR;
        else process.env.LAUNCH_EDITOR = previousLaunchEditor;
      }
      assert(response, "UNC guard did not return an HTTP response");
      assert(controlFilesystemObserved, "filesystem interceptor control did not observe Vite");
      assert(controlChildProcessObserved, "child-process interceptor control did not observe Vite");
      assert(
        response.status === 200 || response.status === 500,
        `UNC guard returned unexpected HTTP ${response.status}`,
      );
      const uncFilesystemProbes = launchSideEffectProbe.filesystemPaths.filter((candidate) =>
        candidate.startsWith("\\\\"),
      ).length;
      const childProcessAttempts = launchSideEffectProbe.childProcessMethods.length;
      assert(uncFilesystemProbes === 0, "launch-editor attempted UNC filesystem access");
      assert(childProcessAttempts === 0, "launch-editor attempted to start a child process");
      return {
        http_status: response.status,
        loopback_unc_only: true,
        interceptor_control_local_filesystem_observed: controlFilesystemObserved,
        interceptor_control_child_process_observed: controlChildProcessObserved,
        unc_filesystem_probes: uncFilesystemProbes,
        child_process_attempts: childProcessAttempts,
        ...sourceGuard,
      };
    },
    { windowsOnly: true, advisories: ["GHSA-v6wh-96g9-6wx3"] },
  );
} finally {
  await server.close();
  rmSync(temporaryRoot, { recursive: true, force: true });
}

const failed = cases.filter((entry) => entry.status === "failed");
const skipped = cases.filter((entry) => entry.status === "skipped");
const coveredAdvisories = [...new Set(cases.flatMap((entry) => entry.advisories))].sort();
const output = {
  contract: CONTRACT,
  version: VERSION,
  ok: failed.length === 0,
  status: failed.length === 0 ? "pass" : "blocked",
  platform: process.platform,
  loopback_only: true,
  synthetic_canaries_only: true,
  advisory_catalog: advisoryCoverage,
  covered_advisories: coveredAdvisories,
  summary: {
    total: cases.length,
    passed: cases.length - failed.length - skipped.length,
    failed: failed.length,
    skipped: skipped.length,
  },
  cases,
};

console.log(JSON.stringify(output));
process.exitCode = failed.length === 0 ? 0 : 1;
