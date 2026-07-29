const HOST_NAME = "com.suzukimakoto.codex_url_vault";

const pageTitle = document.querySelector("#pageTitle");
const pageUrl = document.querySelector("#pageUrl");
const categoryInput = document.querySelector("#categoryInput");
const categoryOptions = document.querySelector("#categoryOptions");
const tagsInput = document.querySelector("#tagsInput");
const noteInput = document.querySelector("#noteInput");
const saveForm = document.querySelector("#saveForm");
const saveButton = document.querySelector("#saveButton");
const status = document.querySelector("#status");
const connectionBadge = document.querySelector("#connectionBadge");

let activeTab = null;

function requestId() {
  return crypto.randomUUID();
}

function sendNativeMessage(message) {
  return new Promise((resolve, reject) => {
    chrome.runtime.sendNativeMessage(HOST_NAME, message, (response) => {
      const transportError = chrome.runtime.lastError;
      if (transportError) {
        reject(new Error(transportError.message));
        return;
      }
      if (!response?.ok) {
        reject(new Error(response?.error?.message || "Native Hostから応答がありません。"));
        return;
      }
      resolve(response.result || {});
    });
  });
}

function isWebUrl(value) {
  try {
    return ["http:", "https:"].includes(new URL(value).protocol);
  } catch {
    return false;
  }
}

function parseTags(value) {
  return [...new Set(
    value
      .replaceAll(";", ",")
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean),
  )];
}

function setStatus(message = "", kind = "") {
  status.textContent = message;
  status.className = `status ${kind}`.trim();
}

function setConnection(connected) {
  connectionBadge.className = `connection-badge ${connected ? "online" : "offline"}`;
  connectionBadge.title = connected ? "Native Host 接続済み" : "Native Host 未接続";
}

async function loadCurrentTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  activeTab = tab || null;
  pageTitle.textContent = tab?.title || "タイトルなし";
  pageUrl.textContent = tab?.url || "";
  if (!isWebUrl(tab?.url || "")) {
    saveButton.disabled = true;
    setStatus("http / https ページだけを保存できます。", "error");
  }
}

async function loadPreferences() {
  const preferences = await chrome.storage.local.get(["lastCategory", "lastTags"]);
  categoryInput.value = preferences.lastCategory || "";
  tagsInput.value = preferences.lastTags || "";
}

async function loadCategories() {
  try {
    const result = await sendNativeMessage({
      type: "list_categories",
      id: requestId(),
    });
    categoryOptions.replaceChildren();
    for (const category of result.categories || []) {
      if (!category.path) continue;
      const option = document.createElement("option");
      option.value = category.path;
      option.label = `${category.display || category.path} (${category.count || 0})`;
      categoryOptions.appendChild(option);
    }
    setConnection(true);
  } catch (error) {
    setConnection(false);
    setStatus(`Native Hostに接続できません: ${error.message}`, "error");
  }
}

async function saveCurrentPage(event) {
  event.preventDefault();
  if (!activeTab || !isWebUrl(activeTab.url || "")) return;
  saveButton.disabled = true;
  setStatus("保存しています…");
  try {
    const result = await sendNativeMessage({
      type: "save_url",
      id: requestId(),
      url: activeTab.url,
      title: activeTab.title || "",
      category: categoryInput.value.trim(),
      tags: parseTags(tagsInput.value),
      note: noteInput.value.trim(),
    });
    await chrome.storage.local.set({
      lastCategory: categoryInput.value.trim(),
      lastTags: tagsInput.value.trim(),
    });
    setConnection(true);
    const savedTitle = result.bookmark?.title || result.bookmark?.url || "URL";
    setStatus(`「${savedTitle}」を保存しました。`, "success");
  } catch (error) {
    setConnection(false);
    setStatus(`保存できません: ${error.message}`, "error");
  } finally {
    saveButton.disabled = false;
  }
}

saveForm.addEventListener("submit", saveCurrentPage);
document.addEventListener("keydown", (event) => {
  if (event.metaKey && event.key === "Enter") {
    saveForm.requestSubmit();
  }
});

Promise.all([loadCurrentTab(), loadPreferences()]).then(loadCategories);
