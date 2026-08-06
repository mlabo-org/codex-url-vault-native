importScripts("browser-config.js");

const HOST_NAME = "com.suzukimakoto.codex_url_vault";
const MENU_ID = "save-to-codex-url-vault";
const NATIVE_TIMEOUT_MS = 8000;
const MAX_RECONNECT_DELAY_MS = 30000;
const BROWSER = globalThis.URL_VAULT_BROWSER;

if (!BROWSER?.id || !BROWSER?.displayName) {
  throw new Error("browser-config.js is invalid");
}

let nativePort = null;
let reconnectTimer = null;
let reconnectDelayMs = 1000;
const pendingNativeRequests = new Map();

class CaptureError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: MENU_ID,
      title: "Codex URL Vaultに保存",
      contexts: ["page", "link"],
    });
  });
  connectNative();
});

chrome.runtime.onStartup.addListener(connectNative);

function connectNative() {
  if (nativePort) return nativePort;
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  const port = chrome.runtime.connectNative(HOST_NAME);
  nativePort = port;
  port.onMessage.addListener((message) => {
    reconnectDelayMs = 1000;
    void handleNativeMessage(message, port);
  });
  port.onDisconnect.addListener(() => {
    const reason = chrome.runtime.lastError?.message || "Native Host disconnected";
    if (nativePort === port) nativePort = null;
    rejectPendingNativeRequests(reason);
    scheduleReconnect();
  });
  return port;
}

function scheduleReconnect() {
  if (reconnectTimer || nativePort) return;
  const delay = reconnectDelayMs;
  reconnectDelayMs = Math.min(reconnectDelayMs * 2, MAX_RECONNECT_DELAY_MS);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectNative();
  }, delay);
}

function rejectPendingNativeRequests(message) {
  for (const { reject, timeout } of pendingNativeRequests.values()) {
    clearTimeout(timeout);
    reject(new Error(message));
  }
  pendingNativeRequests.clear();
}

function nativeRequest(message) {
  return new Promise((resolve, reject) => {
    const port = connectNative();
    const timeout = setTimeout(() => {
      pendingNativeRequests.delete(message.id);
      reject(new Error("Native Host request timed out"));
    }, NATIVE_TIMEOUT_MS);
    pendingNativeRequests.set(message.id, { resolve, reject, timeout });
    try {
      port.postMessage(message);
    } catch (error) {
      clearTimeout(timeout);
      pendingNativeRequests.delete(message.id);
      reject(error);
    }
  });
}

async function handleNativeMessage(message, port) {
  if (message?.type === "capture_current_page") {
    await returnCurrentPage(message.id, port, message.browser);
    return;
  }
  const pending = pendingNativeRequests.get(message?.id);
  if (!pending) return;
  clearTimeout(pending.timeout);
  pendingNativeRequests.delete(message.id);
  if (!message.ok) {
    const error = new Error(message.error?.message || "Native Host error");
    error.code = message.error?.code || "native_host_error";
    pending.reject(error);
    return;
  }
  pending.resolve(message.result || {});
}

async function returnCurrentPage(id, port, requestedBrowser) {
  try {
    if (requestedBrowser && requestedBrowser !== BROWSER.id) {
      throw new CaptureError(
        "browser_mismatch",
        `${BROWSER.displayName} extension cannot satisfy a ${requestedBrowser} capture request.`,
      );
    }
    const page = await captureStableCurrentPage();
    port.postMessage({
      type: "current_page_result",
      id,
      browser: BROWSER.id,
      ok: true,
      url: page.url,
      title: page.title,
      tab_id: page.tabId,
      window_id: page.windowId,
      captured_at: new Date().toISOString(),
    });
  } catch (error) {
    port.postMessage({
      type: "current_page_result",
      id,
      browser: BROWSER.id,
      ok: false,
      error: {
        code: error.code || "capture_failed",
        message: error.message || `現在の${BROWSER.displayName}ページを取得できません。`,
      },
    });
  }
}

async function captureStableCurrentPage() {
  const focusedWindow = await chrome.windows.getLastFocused({
    windowTypes: ["normal"],
  });
  if (!focusedWindow?.id || !focusedWindow.focused) {
    throw new CaptureError(
      "browser_not_focused",
      `${BROWSER.displayName}の通常ウィンドウが前面にありません。`,
    );
  }
  const [tab] = await chrome.tabs.query({
    active: true,
    windowId: focusedWindow.id,
  });
  const initialUrl = webUrl(tab?.url || "");
  if (!tab?.id || !initialUrl) {
    throw new CaptureError(
      "unsupported_url",
      "現在のタブはhttp / httpsページではありません。",
    );
  }

  const [currentWindow, currentTab] = await Promise.all([
    chrome.windows.get(focusedWindow.id),
    chrome.tabs.get(tab.id),
  ]);
  const currentUrl = webUrl(currentTab.url || "");
  if (
    !currentWindow.focused
    || !currentTab.active
    || currentTab.windowId !== focusedWindow.id
    || currentUrl !== initialUrl
  ) {
    throw new CaptureError(
      "capture_target_changed",
      `取得中に${BROWSER.displayName}の対象タブが変わりました。`,
    );
  }
  return {
    url: currentUrl,
    title: currentTab.title || currentUrl,
    tabId: currentTab.id,
    windowId: currentTab.windowId,
  };
}

function webUrl(value) {
  try {
    const parsed = new URL(value);
    return ["http:", "https:"].includes(parsed.protocol) ? parsed.toString() : null;
  } catch {
    return null;
  }
}

async function showBadge(tabId, text, color) {
  await chrome.action.setBadgeBackgroundColor({ tabId, color });
  await chrome.action.setBadgeText({ tabId, text });
  setTimeout(() => chrome.action.setBadgeText({ tabId, text: "" }), 1800);
}

chrome.runtime.onMessage.addListener((request, _sender, sendResponse) => {
  if (request?.type !== "native_request" || !request.message?.id) return false;
  nativeRequest(request.message).then(
    (result) => sendResponse({ ok: true, result }),
    (error) => sendResponse({
      ok: false,
      error: {
        code: error.code || "native_host_error",
        message: error.message || "Native Host error",
      },
    }),
  );
  return true;
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId !== MENU_ID || !tab?.id) return;
  const targetUrl = webUrl(info.linkUrl || info.pageUrl || tab.url || "");
  if (!targetUrl) {
    await showBadge(tab.id, "!", "#c33d4a");
    return;
  }
  const preferences = await chrome.storage.local.get(["lastCategory", "lastTags"]);
  const tags = String(preferences.lastTags || "")
    .replaceAll(";", ",")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
  try {
    await nativeRequest({
      type: "save_url",
      id: crypto.randomUUID(),
      url: targetUrl,
      title: info.linkUrl ? (info.selectionText || targetUrl) : (tab.title || targetUrl),
      category: preferences.lastCategory || "",
      tags: [...new Set(tags)],
      note: "",
      source_browser: BROWSER.id,
    });
    await showBadge(tab.id, "✓", "#16835f");
  } catch {
    await showBadge(tab.id, "!", "#c33d4a");
  }
});

connectNative();
