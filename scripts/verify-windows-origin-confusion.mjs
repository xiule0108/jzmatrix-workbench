import { writeFileSync } from "node:fs";
import { createServer } from "node:http";

const argumentsByName = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  argumentsByName.set(process.argv[index], process.argv[index + 1]);
}

const outputPath = argumentsByName.get("--output") ?? null;
const cdpPort = Number(argumentsByName.get("--cdp-port") ?? "9222");
const remoteHostname = argumentsByName.get("--remote-hostname") ?? "tauri.evil.test";
const appProcessId = Number(argumentsByName.get("--process-id") ?? "0");
const commands = ["doctor", "offline_demo"];
const diagnostics = {
  app_process_id: appProcessId || null,
  app_process_alive: null,
  cdp_port: cdpPort,
  target_poll_attempts: 0,
  cdp_http_reachable: false,
  last_target_error: null,
};

function isProcessAlive(processId) {
  if (!processId) return null;
  try {
    process.kill(processId, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

function emitResult(result) {
  const serialized = `${JSON.stringify(result, null, 2)}\n`;
  if (outputPath) writeFileSync(outputPath, serialized);
  process.stdout.write(serialized);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function waitForTargets() {
  const deadline = Date.now() + 90_000;
  let lastError = null;
  while (Date.now() < deadline) {
    diagnostics.target_poll_attempts += 1;
    diagnostics.app_process_alive = isProcessAlive(appProcessId);
    try {
      const response = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      diagnostics.cdp_http_reachable = true;
      if (response.ok) {
        const targets = await response.json();
        const target = targets.find(
          (entry) => entry.type === "page" && typeof entry.webSocketDebuggerUrl === "string",
        );
        if (target) return target;
      }
    } catch (error) {
      lastError = error;
      diagnostics.last_target_error = error?.message ?? String(error);
    }
    await delay(500);
  }
  throw new Error(`WebView2 CDP target unavailable: ${lastError?.message ?? "timeout"}`);
}

async function connectCdp(url) {
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", () => reject(new Error("WebView2 CDP socket failed")), {
      once: true,
    });
  });

  let nextId = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(String(event.data));
    if (!message.id || !pending.has(message.id)) return;
    const { resolve, reject } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) reject(new Error(message.error.message));
    else resolve(message.result);
  });

  return {
    socket,
    send(method, params = {}) {
      const id = ++nextId;
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
    },
  };
}

async function waitForRemoteDocument(send) {
  const deadline = Date.now() + 30_000;
  let lastObservation = null;
  while (Date.now() < deadline) {
    try {
      const result = await send("Runtime.evaluate", {
        expression:
          "({ hostname: globalThis.location.hostname, readyState: globalThis.document.readyState })",
        returnByValue: true,
      });
      lastObservation = result.result.value;
      if (
        lastObservation?.hostname === remoteHostname &&
        lastObservation?.readyState === "complete"
      ) {
        return lastObservation;
      }
    } catch {
      // The execution context is expected to disappear while Page.navigate replaces it.
    }
    await delay(250);
  }
  throw new Error(`remote document did not load: ${JSON.stringify(lastObservation)}`);
}

const server = createServer((_request, response) => {
  response.writeHead(200, {
    "Content-Type": "text/html; charset=utf-8",
    "Cache-Control": "no-store",
  });
  response.end(
    "<!doctype html><html><head><title>JZMatrix remote-origin probe</title></head>" +
      "<body><main id=probe>remote-origin probe</main></body></html>",
  );
});

await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});

let cdp = null;
let remoteUrl = null;
let stage = "start";
try {
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("probe server address unavailable");
  remoteUrl = `http://${remoteHostname}:${address.port}/`;
  stage = "wait_for_cdp_target";
  const target = await waitForTargets();
  stage = "connect_cdp";
  cdp = await connectCdp(target.webSocketDebuggerUrl);
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  stage = "navigate_remote_origin";
  const navigation = await cdp.send("Page.navigate", { url: remoteUrl });
  if (navigation.errorText) throw new Error(`remote navigation failed: ${navigation.errorText}`);
  const document = await waitForRemoteDocument(cdp.send);

  stage = "invoke_remote_commands";
  const evaluation = await cdp.send("Runtime.evaluate", {
    expression: `
      (async () => {
        const invoke = globalThis.__TAURI_INTERNALS__?.invoke;
        const attempt = async (command) => {
          if (typeof invoke !== "function") {
            return { command, status: "blocked", reason: "tauri_internals_unavailable" };
          }
          return Promise.race([
            invoke(command)
              .then((value) => ({ command, status: "invoked", value }))
              .catch((error) => ({ command, status: "blocked", reason: String(error) })),
            new Promise((resolve) =>
              setTimeout(() => resolve({ command, status: "timeout" }), 10_000),
            ),
          ]);
        };
        return Promise.all(${JSON.stringify(commands)}.map(attempt));
      })()
    `,
    awaitPromise: true,
    returnByValue: true,
  });
  if (evaluation.exceptionDetails) {
    throw new Error(`remote invocation probe threw: ${evaluation.exceptionDetails.text}`);
  }
  const commandResults = evaluation.result.value;
  const passed =
    document.hostname === remoteHostname &&
    commands.every(
      (command) =>
        commandResults.some((result) => result.command === command && result.status === "blocked"),
    );
  const result = {
    contract: "jzmatrix.windows-origin-confusion-probe",
    version: "1.0.0",
    status: passed ? "pass" : "blocked",
    remote_origin: remoteUrl,
    observed_hostname: document.hostname,
    commands: commandResults,
    webview_target: {
      title: target.title,
      initial_url: target.url,
    },
    diagnostics: {
      ...diagnostics,
      app_process_alive: isProcessAlive(appProcessId),
    },
  };
  emitResult(result);
  if (!passed) process.exitCode = 1;
} catch (error) {
  emitResult({
    contract: "jzmatrix.windows-origin-confusion-probe",
    version: "1.0.0",
    status: "blocked",
    error_stage: stage,
    error: error?.message ?? String(error),
    remote_origin: remoteUrl,
    observed_hostname: null,
    commands: commands.map((command) => ({ command, status: "not_run" })),
    diagnostics: {
      ...diagnostics,
      app_process_alive: isProcessAlive(appProcessId),
    },
  });
  process.exitCode = 1;
} finally {
  cdp?.socket.close();
  await new Promise((resolve) => server.close(resolve));
}
