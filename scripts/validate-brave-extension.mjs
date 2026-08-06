import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..");
const hostManifestPath = resolve(
  repoRoot,
  "extension/native-host/com.suzukimakoto.codex_url_vault.json",
);
const hostManifest = JSON.parse(readFileSync(hostManifestPath, "utf8"));
const browsers = [
  { id: "brave", displayName: "Brave" },
  { id: "chrome", displayName: "Google Chrome" },
];
const sharedFiles = [
  "icons/icon-16.png",
  "icons/icon-32.png",
  "icons/icon-48.png",
  "icons/icon-128.png",
  "popup.css",
  "popup.html",
  "popup.js",
  "service-worker.js",
];

function fail(message) {
  throw new Error(message);
}

function extensionIdFromKey(key) {
  const digest = createHash("sha256")
    .update(Buffer.from(key, "base64"))
    .digest()
    .subarray(0, 16);
  const alphabet = "abcdefghijklmnop";
  return [...digest]
    .flatMap((byte) => [alphabet[byte >> 4], alphabet[byte & 15]])
    .join("");
}

function validateBrowserExtension(browser) {
  const extensionRoot = resolve(repoRoot, `extension/${browser.id}`);
  const manifest = JSON.parse(
    readFileSync(resolve(extensionRoot, "manifest.json"), "utf8"),
  );
  const configSource = readFileSync(
    resolve(extensionRoot, "browser-config.js"),
    "utf8",
  );

  if (manifest.manifest_version !== 3) fail(`${browser.id}: manifest_version must be 3`);
  if (!manifest.key) fail(`${browser.id}: manifest.key is required for a stable extension ID`);
  if (!manifest.name.includes(browser.id === "chrome" ? "Chrome" : "Brave")) {
    fail(`${browser.id}: manifest name does not identify the browser package`);
  }
  if (!configSource.includes(`id: "${browser.id}"`)) {
    fail(`${browser.id}: browser-config.js has the wrong browser ID`);
  }
  if (!configSource.includes(`displayName: "${browser.displayName}"`)) {
    fail(`${browser.id}: browser-config.js has the wrong display name`);
  }

  const extensionId = extensionIdFromKey(manifest.key);
  const expectedOrigin = `chrome-extension://${extensionId}/`;
  if (!hostManifest.allowed_origins?.includes(expectedOrigin)) {
    fail(`${browser.id}: Native Host is not bound to ${expectedOrigin}`);
  }
  if (hostManifest.name !== "com.suzukimakoto.codex_url_vault") {
    fail("Native Host name does not match the extensions");
  }
  if (!hostManifest.path?.startsWith("/")) {
    fail("Native Host template path must be absolute");
  }

  const expectedPermissions = new Set([
    "contextMenus",
    "nativeMessaging",
    "storage",
    "tabs",
  ]);
  for (const permission of manifest.permissions || []) {
    if (!expectedPermissions.delete(permission)) {
      fail(`${browser.id}: unexpected extension permission: ${permission}`);
    }
  }
  if (expectedPermissions.size) {
    fail(`${browser.id}: missing extension permissions: ${[...expectedPermissions].join(", ")}`);
  }
  if (manifest.host_permissions?.length) {
    fail(`${browser.id}: extension must not request broad host permissions`);
  }

  const requiredFiles = [
    "browser-config.js",
    manifest.action?.default_popup,
    manifest.background?.service_worker,
    ...Object.values(manifest.icons || {}),
    ...Object.values(manifest.action?.default_icon || {}),
  ].filter(Boolean);
  for (const relativePath of new Set(requiredFiles)) {
    if (!existsSync(resolve(extensionRoot, relativePath))) {
      fail(`${browser.id}: missing extension artifact: ${relativePath}`);
    }
  }

  const serviceWorker = readFileSync(
    resolve(extensionRoot, "service-worker.js"),
    "utf8",
  );
  for (const requiredSource of [
    hostManifest.name,
    "importScripts(\"browser-config.js\")",
    "chrome.runtime.connectNative",
    "capture_current_page",
    "current_page_result",
    "browser: BROWSER.id",
    "source_browser: BROWSER.id",
    "chrome.windows.getLastFocused",
    "chrome.tabs.get",
    "capture_target_changed",
  ]) {
    if (!serviceWorker.includes(requiredSource)) {
      fail(`${browser.id}: service-worker.js is missing ${requiredSource}`);
    }
  }
  if (serviceWorker.includes("chrome.runtime.sendNativeMessage")) {
    fail(`${browser.id}: service-worker.js must use the persistent native port`);
  }

  const popup = readFileSync(resolve(extensionRoot, "popup.js"), "utf8");
  if (!popup.includes("chrome.runtime.sendMessage")) {
    fail(`${browser.id}: popup.js must route Native Host requests through the service worker`);
  }
  if (!popup.includes("source_browser: BROWSER.id")) {
    fail(`${browser.id}: popup.js must identify its source browser`);
  }
  if (popup.includes("chrome.runtime.sendNativeMessage")) {
    fail(`${browser.id}: popup.js must not launch one-shot Native Host processes`);
  }
  return extensionId;
}

const extensionIds = new Map(
  browsers.map((browser) => [browser.id, validateBrowserExtension(browser)]),
);
if (new Set(extensionIds.values()).size !== 1) {
  fail("Brave and Chrome packages must keep the same stable extension ID");
}
for (const relativePath of sharedFiles) {
  const braveBytes = readFileSync(resolve(repoRoot, "extension/brave", relativePath));
  const chromeBytes = readFileSync(resolve(repoRoot, "extension/chrome", relativePath));
  if (!braveBytes.equals(chromeBytes)) {
    fail(`shared extension file differs between browsers: ${relativePath}`);
  }
}

console.log(
  `Browser extension validation passed: Brave=${extensionIds.get("brave")} Chrome=${extensionIds.get("chrome")}`,
);
