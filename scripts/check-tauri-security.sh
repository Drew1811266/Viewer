#!/usr/bin/env bash

set -euo pipefail

ROOT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIRECTORY"

node --input-type=module <<'NODE'
import { readFileSync } from "node:fs";

const capability = JSON.parse(readFileSync("src-tauri/capabilities/main.json", "utf8"));
const configuration = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const allowedPermissions = new Set([
  "dialog:allow-open",
  "core:event:allow-listen",
  "core:event:allow-unlisten",
]);
const forbiddenPrefixes = [
  "fs:",
  "shell:",
  "sql:",
  "http:",
  "updater:",
  "websocket:",
  "upload:",
];

if (JSON.stringify(capability.windows) !== JSON.stringify(["main"])) {
  throw new Error("main capability must be scoped to the main window only");
}
if (Object.hasOwn(capability, "remote")) {
  throw new Error("remote capability origins are forbidden");
}
for (const permission of capability.permissions ?? []) {
  const identifier = typeof permission === "string" ? permission : permission.identifier;
  if (typeof identifier !== "string" || !allowedPermissions.has(identifier)) {
    throw new Error(`permission is outside the Viewer allowlist: ${String(identifier)}`);
  }
  if (forbiddenPrefixes.some((prefix) => identifier.startsWith(prefix))) {
    throw new Error(`forbidden capability prefix: ${identifier}`);
  }
}
if ((capability.permissions ?? []).length !== allowedPermissions.size) {
  throw new Error("main capability must contain the exact reviewed permission set");
}

const csp = configuration?.app?.security?.csp;
if (typeof csp !== "string") {
  throw new Error("Tauri CSP must be a string");
}
const directives = new Map(
  csp
    .split(";")
    .map((entry) => entry.trim().split(/\s+/).filter(Boolean))
    .filter((parts) => parts.length > 0)
    .map(([name, ...sources]) => [name, sources]),
);
for (const [name, sources] of directives) {
  for (const source of sources) {
    if (source === "*" || source.includes("*")) {
      throw new Error(`wildcard CSP source in ${name}: ${source}`);
    }
    if (source === "'unsafe-eval'") {
      throw new Error(`unsafe-eval is forbidden in ${name}`);
    }
    if (source.startsWith("https:")) {
      throw new Error(`remote HTTPS source in ${name}: ${source}`);
    }
    const isTauriIpc = name === "connect-src" && source === "http://ipc.localhost";
    const isMacCustomImage = name === "img-src" && source === "http://viewer-image.localhost";
    if (source.startsWith("http:") && !isTauriIpc && !isMacCustomImage) {
      throw new Error(`remote HTTP source in ${name}: ${source}`);
    }
  }
}
const connectSources = directives.get("connect-src") ?? [];
if (JSON.stringify(connectSources) !== JSON.stringify(["'self'", "ipc:", "http://ipc.localhost"])) {
  throw new Error(`connect-src exceeds Tauri IPC: ${connectSources.join(" ")}`);
}
for (const [name, sources] of directives) {
  if (
    name !== "img-src" &&
    (sources.includes("viewer-image:") || sources.includes("http://viewer-image.localhost"))
  ) {
    throw new Error(`viewer-image scheme is forbidden in ${name}`);
  }
}
if (!(directives.get("img-src") ?? []).includes("viewer-image:")) {
  throw new Error("img-src must contain the restricted viewer-image scheme");
}

console.log("Tauri capability and CSP allowlists passed");
NODE

cargo test --locked -p viewer-desktop --test security_boundaries

printf 'Viewer Tauri security boundary passed\n'
