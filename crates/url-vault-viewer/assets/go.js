const params = new URLSearchParams(window.location.search);
const targetUrl = params.get("url") || "";
const targetTitle = params.get("title") || targetUrl || "Bookmark";
const rid = params.get("rid") || `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
const launchKey = `bookmarkVaultLaunch:${rid}`;

if (document.documentElement.dataset.agentHost === "claude_code") {
  document.querySelector(".eyebrow").textContent = "Opening Here";
}
document.querySelector("#launchTitle").textContent = targetTitle;
document.querySelector("#launchUrl").textContent = targetUrl;

function isSafeHttpUrl(value) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

function returnToVault() {
  sessionStorage.removeItem(launchKey);
  window.location.replace("./index.html");
}

function launchOrReturn() {
  if (!isSafeHttpUrl(targetUrl)) {
    returnToVault();
    return;
  }

  if (sessionStorage.getItem(launchKey) === "launched") {
    returnToVault();
    return;
  }

  sessionStorage.setItem(launchKey, "launched");
  window.location.assign(targetUrl);
}

window.addEventListener("pageshow", launchOrReturn);
