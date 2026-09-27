const params = new URLSearchParams(window.location.search);
const targetUrl = params.get("url") || "";
const targetTitle = params.get("title") || targetUrl || "Bookmark";

const backVault = document.querySelector("#backVault");
const readerTitle = document.querySelector("#readerTitle");
const readerUrl = document.querySelector("#readerUrl");
const openDirect = document.querySelector("#openDirect");
const openExternal = document.querySelector("#openExternal");
const siteFrame = document.querySelector("#siteFrame");

if (document.documentElement.dataset.agentHost === "claude_code") {
  openDirect.textContent = "Open here";
  document.querySelector(".frame-notice").textContent =
    "Reader Preview fetches a simplified local copy. Use Open here or your default browser for the original page.";
}
readerTitle.textContent = targetTitle;
readerUrl.textContent = targetUrl;

function isSafeHttpUrl(value) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

function openInIab() {
  if (!isSafeHttpUrl(targetUrl)) return;
  const rid = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  const params = new URLSearchParams({ url: targetUrl, title: targetTitle, rid });
  window.location.assign(`./go.html?${params.toString()}`);
}

async function openInDefaultBrowser() {
  if (!isSafeHttpUrl(targetUrl)) return;
  if (location.protocol === "file:") {
    alert("Default browser open requires the localhost viewer server.");
    return;
  }
  await fetch(`/open-external?url=${encodeURIComponent(targetUrl)}`);
}

backVault.addEventListener("click", (event) => {
  if (!document.referrer.includes("index.html") && history.length <= 1) return;
  event.preventDefault();
  history.back();
});

openDirect.addEventListener("click", openInIab);
openExternal.addEventListener("click", openInDefaultBrowser);

if (isSafeHttpUrl(targetUrl)) {
  siteFrame.src = `/reader?url=${encodeURIComponent(targetUrl)}&title=${encodeURIComponent(targetTitle)}`;
} else {
  openDirect.disabled = true;
  openExternal.disabled = true;
}
