const HOST_NAME = "com.suzukimakoto.codex_url_vault";
const MENU_ID = "save-to-codex-url-vault";

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: MENU_ID,
    title: "Codex URL Vaultに保存",
    contexts: ["page", "link"],
  });
});

function sendNativeMessage(message) {
  return new Promise((resolve, reject) => {
    chrome.runtime.sendNativeMessage(HOST_NAME, message, (response) => {
      const transportError = chrome.runtime.lastError;
      if (transportError) {
        reject(new Error(transportError.message));
        return;
      }
      if (!response?.ok) {
        reject(new Error(response?.error?.message || "Native Host error"));
        return;
      }
      resolve(response.result || {});
    });
  });
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
    await sendNativeMessage({
      type: "save_url",
      id: crypto.randomUUID(),
      url: targetUrl,
      title: info.linkUrl ? (info.selectionText || targetUrl) : (tab.title || targetUrl),
      category: preferences.lastCategory || "",
      tags: [...new Set(tags)],
      note: "",
    });
    await showBadge(tab.id, "✓", "#16835f");
  } catch {
    await showBadge(tab.id, "!", "#c33d4a");
  }
});
