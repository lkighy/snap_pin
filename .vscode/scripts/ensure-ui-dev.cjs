const { spawn } = require("node:child_process");
const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");

const root = path.resolve(__dirname, "..", "..");
const uiDir = path.join(root, "apps", "tauri_desktop", "ui");
const url = "http://127.0.0.1:1420/";
const logDir = path.join(root, ".vscode", "task-logs");
const logFile = path.join(logDir, "ui-dev.log");

function isUiServerReady() {
  return new Promise((resolve) => {
    const req = http.get(url, (res) => {
      res.resume();
      resolve(res.statusCode >= 200 && res.statusCode < 500);
    });

    req.setTimeout(1000, () => {
      req.destroy();
      resolve(false);
    });

    req.on("error", () => resolve(false));
  });
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main() {
  if (await isUiServerReady()) {
    console.log(`UI dev server is already running at ${url}`);
    return;
  }

  fs.mkdirSync(logDir, { recursive: true });
  const log = fs.openSync(logFile, "a");

  console.log(`Starting UI dev server at ${url}`);
  const child = spawn("pnpm", ["--dir", uiDir, "dev"], {
    cwd: root,
    detached: true,
    stdio: ["ignore", log, log],
    windowsHide: true,
    shell: process.platform === "win32",
  });
  child.unref();

  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (await isUiServerReady()) {
      console.log(`UI dev server is ready at ${url}`);
      return;
    }

    await wait(500);
  }

  throw new Error(`Timed out waiting for UI dev server at ${url}. See ${logFile} for details.`);
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
