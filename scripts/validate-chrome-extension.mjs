import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..");
const extensionRoot = resolve(repoRoot, "extension/chrome");
const manifestPath = resolve(extensionRoot, "manifest.json");
const hostManifestPath = resolve(
  repoRoot,
  "extension/native-host/com.suzukimakoto.codex_url_vault.json",
);

const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const hostManifest = JSON.parse(readFileSync(hostManifestPath, "utf8"));

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

if (manifest.manifest_version !== 3) fail("manifest_version must be 3");
if (!manifest.key) fail("manifest.key is required for a stable extension ID");

const extensionId = extensionIdFromKey(manifest.key);
const expectedOrigin = `chrome-extension://${extensionId}/`;
if (!hostManifest.allowed_origins?.includes(expectedOrigin)) {
  fail(`Native Host is not bound to ${expectedOrigin}`);
}
if (hostManifest.name !== "com.suzukimakoto.codex_url_vault") {
  fail("Native Host name does not match the extension");
}
if (!hostManifest.path?.startsWith("/")) {
  fail("Native Host path must be absolute");
}

const expectedPermissions = new Set([
  "activeTab",
  "contextMenus",
  "nativeMessaging",
  "storage",
]);
for (const permission of manifest.permissions || []) {
  if (!expectedPermissions.delete(permission)) {
    fail(`Unexpected extension permission: ${permission}`);
  }
}
if (expectedPermissions.size) {
  fail(`Missing extension permissions: ${[...expectedPermissions].join(", ")}`);
}
if (manifest.host_permissions?.length) {
  fail("Chrome extension must not request broad host permissions");
}

const requiredFiles = [
  manifest.action?.default_popup,
  manifest.background?.service_worker,
  ...Object.values(manifest.icons || {}),
  ...Object.values(manifest.action?.default_icon || {}),
].filter(Boolean);
for (const relativePath of new Set(requiredFiles)) {
  if (!existsSync(resolve(extensionRoot, relativePath))) {
    fail(`Missing extension artifact: ${relativePath}`);
  }
}

for (const script of ["popup.js", "service-worker.js"]) {
  const source = readFileSync(resolve(extensionRoot, script), "utf8");
  if (!source.includes(hostManifest.name)) {
    fail(`${script} does not reference ${hostManifest.name}`);
  }
}

console.log(`Chrome extension validation passed: ${extensionId}`);
