let bookmarks = [];
let categories = [];

const state = {
  folder: "All",
  query: "",
};
let restoredFromSession = false;

const grid = document.querySelector("#grid");
const folderList = document.querySelector("#folderList");
const searchInput = document.querySelector("#searchInput");
const visibleCount = document.querySelector("#visibleCount");
const totalCount = document.querySelector("#totalCount");
const viewTitle = document.querySelector("#viewTitle");
const emptyState = document.querySelector("#emptyState");
const importOnboarding = document.querySelector("#importOnboarding");
const importBookmarksButton = document.querySelector("#importBookmarksButton");
const onboardingImportButton = document.querySelector("#onboardingImportButton");
const addUrlButton = document.querySelector("#addUrlButton");
const addCategoryButton = document.querySelector("#addCategoryButton");
const openVaultExternalButton = document.querySelector("#openVaultExternalButton");
const promptSamplesButton = document.querySelector("#promptSamplesButton");
const promptSamplesDialog = document.querySelector("#promptSamplesDialog");
const promptSamplesList = document.querySelector("#promptSamplesList");
const copyAllPromptsButton = document.querySelector("#copyAllPromptsButton");
const clearFiltersButton = document.querySelector("#clearFiltersButton");
const dialog = document.querySelector("#bookmarkDialog");
const form = document.querySelector("#bookmarkForm");
const sidebarResizeHandle = document.querySelector("#sidebarResizeHandle");
const deleteBookmarkButton = document.querySelector("#deleteBookmarkButton");
const categoryDialog = document.querySelector("#categoryDialog");
const categoryForm = document.querySelector("#categoryForm");
const categoryPath = document.querySelector("#categoryPath");
const categoryDialogTitle = document.querySelector("#categoryDialogTitle");
const deleteCategoryButton = document.querySelector("#deleteCategoryButton");
const categoryDeleteConfirm = document.querySelector("#categoryDeleteConfirm");
const confirmCategoryDeleteButton = document.querySelector("#confirmCategoryDeleteButton");
const snapshotDialog = document.querySelector("#snapshotDialog");
const snapshotTitle = document.querySelector("#snapshotTitle");
const snapshotMeta = document.querySelector("#snapshotMeta");
const snapshotContent = document.querySelector("#snapshotContent");
const importDialog = document.querySelector("#importDialog");
const importForm = document.querySelector("#importForm");
const importStatus = document.querySelector("#importStatus");
const importFile = document.querySelector("#importFile");
const importDropZone = document.querySelector("#importDropZone");
const selectedFileName = document.querySelector("#selectedFileName");
const stripCommonRoot = document.querySelector("#stripCommonRoot");
const filePreview = document.querySelector("#filePreview");
const importBackButton = document.querySelector("#importBackButton");
const importNextButton = document.querySelector("#importNextButton");
const importRunButton = document.querySelector("#importRunButton");
const importDoneButton = document.querySelector("#importDoneButton");
const importCancelButton = document.querySelector("#importCancelButton");
const resetConfirm = document.querySelector("#resetConfirm");
const resetConfirmRow = document.querySelector("#resetConfirmRow");
const importResult = document.querySelector("#importResult");
const STATE_KEY = "bookmarkVaultState";
const SIDEBAR_WIDTH_KEY = "bookmarkVaultSidebarWidth";
const PREFERENCES_KEY = "codexUrlVaultPreferences";
const LEGACY_THEME_KEY = "bookmarkVaultTheme";
const THEMES = ["system", "light", "dark"];
const LANGUAGES = ["system", "ja", "en"];
const MAX_IMPORT_FILE_BYTES = 20 * 1024 * 1024;
const AGENT_HOST = document.documentElement.dataset.agentHost === "claude_code" ? "claude_code" : "codex";
let editingCategoryPath = null;
const importState = {
  step: 1,
  file: null,
  text: "",
  preview: null,
  result: null,
  busy: false,
  statusKey: "",
  statusKind: "info",
};
const MESSAGES = {
  ja: {
    brandSubtitle: "Codex用のローカルURL保管庫",
    searchLabel: "検索",
    searchPlaceholder: "mlabo, SWELL, Cloudflare...",
    folders: "フォルダ",
    localIndex: "ローカルURL索引",
    allBookmarks: "すべてのブックマーク",
    languageControl: "言語",
    themeControl: "テーマ",
    openInDefaultBrowser: "既定ブラウザでVaultを開く",
    promptSamples: "Codexへの依頼例",
    promptSamplesHelp: "Codexには自然な言葉で依頼できます。スキル呼び出し記法やMCPツール名の入力は不要です。各例には、プラグインが内部で選ぶ主なMCPルートを併記しています。",
    mcpRoute: "主なMCP",
    addUrl: "URLを追加",
    addCategory: "カテゴリを追加",
    importBookmarks: "ブックマークをインポート",
    firstSetup: "最初のセットアップ",
    bringBookmarks: "いまのブックマークを、そのままVaultへ",
    bringBookmarksHelp: "ブラウザからHTMLを書き出し、内容を確認してから安全に取り込みます。",
    importSteps: "インポート手順",
    chooseBrowser: "ブラウザを選ぶ",
    chooseHtml: "HTMLを選ぶ",
    reviewImport: "確認して取り込む",
    startImport: "インポートを始める",
    browserMigration: "ブラウザ移行",
    importProgress: "インポートの進行状況",
    browser: "ブラウザ",
    htmlFile: "HTMLファイル",
    confirm: "確認",
    whichBrowser: "どのブラウザから移しますか？",
    browserChoiceHelp: "選ぶと、HTMLを書き出す手順を表示します。",
    otherBrowser: "その他",
    exportHtml: "HTMLを書き出す",
    exportGuideNote: "書き出したHTMLは次の画面で選びます。元のブラウザのブックマークは変更されません。",
    profileOptional: "プロファイル名（任意）",
    selectExportedHtml: "書き出したHTMLを選択",
    filePrivacy: "ファイルはこのMac内のローカルVaultだけで処理されます。",
    dropFile: "ここにドロップ、またはファイルを選択",
    htmlOnly: "ブラウザから書き出したHTML（最大20MB）",
    stripRoot: "一番外側の共通フォルダを外す",
    stripRootHelp: "書き出し元が全項目を1つのフォルダで包んでいる場合だけ使います。",
    bookmarksFound: "検出したURL",
    foldersFound: "フォルダ",
    duplicatesFound: "重複URL",
    chooseImportMethod: "取り込み方法を確認",
    noAutomaticDelete: "どの通常モードでも、HTMLに無い既存URLを自動削除しません。",
    importMethod: "取り込み方法",
    differentialSync: "差分同期",
    recommended: "推奨",
    differentialSyncHelp: "新規URLを追加し、同じURLの情報を更新します。HTMLに無いURLは残します。",
    addOnly: "追加のみ",
    addOnlyHelp: "新規URLだけ追加し、既存URLのタイトルや整理内容には触れません。",
    advancedReplace: "詳細設定：すべて置き換え",
    replaceAll: "すべて置き換え",
    replaceAllHelp: "先にDBをバックアップし、現在のVaultデータを消してこのHTMLを新しい基準にします。",
    replaceConfirm: "現在のVaultデータが置き換わることを理解しました",
    source: "移行元",
    file: "ファイル",
    importCount: "取り込み対象",
    importComplete: "インポート完了",
    added: "追加",
    updated: "更新",
    skipped: "変更なし",
    back: "戻る",
    next: "次へ",
    runImport: "インポートする",
    viewBookmarks: "ブックマークを見る",
    chooseBrowserError: "移行元のブラウザを選んでください。",
    chooseFileError: "ブラウザから書き出したHTMLを選んでください。",
    invalidFileError: "HTMLファイル（.html / .htm）を選んでください。",
    fileTooLargeError: "ファイルが20MBを超えています。",
    analyzingFile: "HTMLの内容を確認しています…",
    importingFile: "Vaultへ取り込んでいます…",
    resetConfirmError: "すべて置き換えるには、確認欄にチェックしてください。",
    importReady: "内容を確認できました。次へ進めます。",
    importFailed: "インポートできませんでした。ファイルを確認してもう一度お試しください。",
    unrecognizedHtml: "ブラウザから書き出したブックマークHTMLとして認識できませんでした。",
    noBookmarksInHtml: "このHTMLからブックマークURLを検出できませんでした。",
    previewChanged: "確認後にファイル内容が変わりました。もう一度選択してください。",
    importedTitle: "{count}件を確認しました",
    mergeResult: "差分同期が完了しました。HTMLに無い既存URLは残しています。",
    addResult: "追加のみで取り込みました。既存URLの情報は変更していません。",
    resetResult: "置き換えが完了しました。実行前のDBバックアップも保存されています。",
    noMatches: "該当なし",
    noMatchesHelp: "別の語句、フォルダ、エイリアス、URL断片を試してください。",
    clearFilters: "絞り込みを解除",
    editUrl: "URLを編集",
    title: "タイトル",
    category: "カテゴリ",
    aliases: "エイリアス",
    tags: "タグ",
    intents: "用途",
    note: "メモ",
    delete: "削除",
    cancel: "キャンセル",
    save: "保存",
    close: "閉じる",
    copy: "コピー",
    copyAll: "すべてコピー",
    copied: "コピー済み",
    iab: "IAB",
    preview: "元ページのプレビュー",
    defaultBrowser: "既定ブラウザ",
    brave: "Brave",
    edit: "編集",
    savedCopy: "保存版",
    savedSnapshot: "保存済みスナップショット",
    snapshotUnavailable: "保存内容を表示できません",
    snapshotStatus: "状態",
    snapshotKind: "種類",
    capturedAt: "保存日時",
    vaultUnavailable: "Vaultを利用できません",
    deleteConfirm: "このURLをVaultから削除しますか？",
    newCategoryPath: "新しいカテゴリのパス",
    categoryPathPlaceholder: "例: Research/AI",
    categoryPathHelp: "スラッシュで階層を作れます。",
    editCategory: "カテゴリを編集",
    deleteCategory: "カテゴリを削除",
    deleteCategoryTitle: "このカテゴリを削除しますか？",
    deleteCategoryHelp: "中のURLは削除されず、「未分類」へ移動します。",
    confirmDeleteCategory: "カテゴリを削除",
    openItem: "開く",
    all: "すべて",
    unfiled: "未分類",
  },
  en: {
    brandSubtitle: "Local URL preservation for Codex",
    searchLabel: "Search",
    searchPlaceholder: "mlabo, SWELL, Cloudflare...",
    folders: "Folders",
    localIndex: "Local URL Index",
    allBookmarks: "All bookmarks",
    languageControl: "Language",
    themeControl: "Theme",
    openInDefaultBrowser: "Open Vault in default browser",
    promptSamples: "Example requests for Codex",
    promptSamplesHelp: "Ask Codex in natural language. You do not need to enter a skill invocation or MCP tool name. Each example also shows the main MCP route the plugin selects internally.",
    mcpRoute: "Primary MCP",
    addUrl: "Add URL",
    addCategory: "Add Category",
    importBookmarks: "Import Bookmarks",
    firstSetup: "First-time setup",
    bringBookmarks: "Bring your existing bookmarks into the Vault",
    bringBookmarksHelp: "Export HTML from your browser, review it, then import it safely.",
    importSteps: "Import steps",
    chooseBrowser: "Choose browser",
    chooseHtml: "Choose HTML",
    reviewImport: "Review and import",
    startImport: "Start import",
    browserMigration: "Browser migration",
    importProgress: "Import progress",
    browser: "Browser",
    htmlFile: "HTML file",
    confirm: "Review",
    whichBrowser: "Which browser are you moving from?",
    browserChoiceHelp: "Choose one to see its HTML export steps.",
    otherBrowser: "Other",
    exportHtml: "Export HTML",
    exportGuideNote: "Choose the exported HTML on the next screen. Your original browser bookmarks are not changed.",
    profileOptional: "Profile name (optional)",
    selectExportedHtml: "Select the exported HTML",
    filePrivacy: "The file is processed only by the local Vault on this Mac.",
    dropFile: "Drop it here or choose a file",
    htmlOnly: "Browser-exported HTML (up to 20 MB)",
    stripRoot: "Remove the outer common folder",
    stripRootHelp: "Use only when the export wraps every item in one extra folder.",
    bookmarksFound: "URLs found",
    foldersFound: "Folders",
    duplicatesFound: "Duplicate URLs",
    chooseImportMethod: "Review the import method",
    noAutomaticDelete: "Normal import modes never delete existing URLs missing from this HTML.",
    importMethod: "Import method",
    differentialSync: "Differential sync",
    recommended: "Recommended",
    differentialSyncHelp: "Adds new URLs and updates matching URLs. URLs absent from the HTML remain in the Vault.",
    addOnly: "Add only",
    addOnlyHelp: "Adds only new URLs and preserves all organization on existing URLs.",
    advancedReplace: "Advanced: replace everything",
    replaceAll: "Replace everything",
    replaceAllHelp: "Backs up the database, clears current Vault data, and uses this HTML as the new baseline.",
    replaceConfirm: "I understand that current Vault data will be replaced",
    source: "Source",
    file: "File",
    importCount: "Ready to import",
    importComplete: "Import complete",
    added: "Added",
    updated: "Updated",
    skipped: "Unchanged",
    back: "Back",
    next: "Next",
    runImport: "Import",
    viewBookmarks: "View bookmarks",
    chooseBrowserError: "Choose the source browser.",
    chooseFileError: "Choose the HTML exported from your browser.",
    invalidFileError: "Choose an HTML file (.html or .htm).",
    fileTooLargeError: "The file is larger than 20 MB.",
    analyzingFile: "Checking the HTML…",
    importingFile: "Importing into the Vault…",
    resetConfirmError: "Confirm the replacement before continuing.",
    importReady: "The file is ready. You can continue.",
    importFailed: "The import failed. Check the file and try again.",
    unrecognizedHtml: "This is not a recognized browser bookmark HTML file.",
    noBookmarksInHtml: "No bookmark URLs were found in this HTML.",
    previewChanged: "The file changed after review. Select it again.",
    importedTitle: "Reviewed {count} bookmarks",
    mergeResult: "Differential sync is complete. Existing URLs absent from the HTML were kept.",
    addResult: "Add-only import is complete. Existing URL details were not changed.",
    resetResult: "Replacement is complete. A pre-import database backup was also saved.",
    noMatches: "No matches",
    noMatchesHelp: "Try another term, folder, alias, or URL fragment.",
    clearFilters: "Clear filters",
    editUrl: "Edit URL",
    title: "Title",
    category: "Category",
    aliases: "Aliases",
    tags: "Tags",
    intents: "Intents",
    note: "Note",
    delete: "Delete",
    cancel: "Cancel",
    save: "Save",
    close: "Close",
    copy: "Copy",
    copyAll: "Copy all",
    copied: "Copied",
    iab: "IAB",
    preview: "Source preview",
    defaultBrowser: "Default browser",
    brave: "Brave",
    edit: "Edit",
    savedCopy: "Saved copy",
    savedSnapshot: "Saved snapshot",
    snapshotUnavailable: "Saved content is unavailable",
    snapshotStatus: "Status",
    snapshotKind: "Kind",
    capturedAt: "Captured",
    vaultUnavailable: "Vault unavailable",
    deleteConfirm: "Delete this URL from the Vault?",
    newCategoryPath: "New category path",
    categoryPathPlaceholder: "Example: Research/AI",
    categoryPathHelp: "Use slashes to create nested categories.",
    editCategory: "Edit Category",
    deleteCategory: "Delete Category",
    deleteCategoryTitle: "Delete this category?",
    deleteCategoryHelp: "Its URLs will not be deleted. They will move to Unfiled.",
    confirmDeleteCategory: "Delete Category",
    openItem: "Open",
    all: "All",
    unfiled: "Unfiled",
  },
};
const HOST_MESSAGES = {
  codex: { ja: {}, en: {} },
  claude_code: {
    ja: {
      brandSubtitle: "Claude Code用のローカルURL保管庫",
      promptSamples: "Claude Codeへの依頼例",
      promptSamplesHelp: "Claude Codeには自然な言葉で依頼できます。スキル呼び出し記法やMCPツール名の入力は不要です。各例には、プラグインが内部で選ぶ主なMCPルートを併記しています。",
      iab: "ここで開く",
    },
    en: {
      brandSubtitle: "Local URL preservation for Claude Code",
      promptSamples: "Example requests for Claude Code",
      promptSamplesHelp: "Ask Claude Code in natural language. You do not need to enter a skill invocation or MCP tool name. Each example also shows the main MCP route the plugin selects internally.",
      iab: "Open here",
    },
  },
};
let preferences = readPreferences();
const PROMPT_SAMPLES = [
  {
    title: { ja: "ネイティブアプリを開く", en: "Open the native app" },
    route: "show_vault",
    prompt: { ja: "Codex URL Vaultのネイティブアプリを開いて。", en: "Open the native Codex URL Vault app." },
  },
  AGENT_HOST === "claude_code"
    ? {
      title: { ja: "Claude Codeのブラウザペインでカード表示する", en: "Show the card view in the Claude Code browser pane" },
      route: "show_iab_vault → Claude Code browser pane",
      prompt: { ja: "Codex URL VaultをClaude Codeのブラウザペインでカード表示して。", en: "Open Codex URL Vault as a card view in the Claude Code browser pane." },
    }
    : {
      title: { ja: "Codex内でカード表示する", en: "Show the card view inside Codex" },
      route: "show_iab_vault → Codex Browser",
      prompt: { ja: "Codex URL VaultをCodex内のカード表示で開いて。", en: "Open Codex URL Vault as a card view inside Codex." },
    },
  {
    title: { ja: "詳細を付けてURLを保存する", en: "Save a URL with details" },
    route: "save_url",
    prompt: { ja: "Codex URL Vaultに https://example.com を保存して。タイトルは「Example」、カテゴリは「Research/Web」、エイリアスは「example」、タグは「reference」と「sample」、用途は「documentation」にして。", en: "Save https://example.com in Codex URL Vault with the title “Example,” category “Research/Web,” alias “example,” tags “reference” and “sample,” and intent “documentation.”" },
  },
  {
    title: { ja: "保存済みURLを検索する", en: "Search saved URLs" },
    route: "search_urls",
    prompt: { ja: "Codex URL Vaultで「Cloudflare SSL」を検索して、関連度の高い候補を3件、タイトル・URL・カテゴリ付きで見せて。まだ開かないで。", en: "Search Codex URL Vault for “Cloudflare SSL” and show the three most relevant candidates with title, URL, and category. Do not open anything yet." },
  },
  {
    title: { ja: "あいまいに探してから開く", en: "Resolve a vague request, then open" },
    route: "suggest_urls → open_url",
    prompt: { ja: "Codex URL Vaultで「CloudflareのSSL設定」を探して。候補が複数ある場合は一覧を出し、私が選んだURLだけをBraveで開いて。", en: "Find my saved link about Cloudflare SSL configuration in Codex URL Vault. If there are multiple candidates, show them first and open only the one I select in Brave." },
  },
  {
    title: { ja: "貼り付けた本文を保存版として残す", en: "Preserve supplied text" },
    route: "save_snapshot",
    prompt: { ja: "Codex URL Vaultに保存済みの https://example.com へ、次のMarkdown本文を保存版として記録して。Webページは取得せず、この本文をそのまま保存して。\n\n# Example\n確認済みの要点です。", en: "For https://example.com in Codex URL Vault, preserve the following Markdown as its saved copy. Do not fetch the web page; save this text verbatim.\n\n# Example\nThese are the verified notes." },
  },
  {
    title: { ja: "ブックマークを確認してから取り込む", en: "Preview bookmarks before importing" },
    route: "preview_bookmark_import → apply_bookmark_import",
    prompt: { ja: "~/Downloads/bookmarks.html をCodex URL Vaultへ取り込む前にプレビューして、追加・重複・エラー件数を見せて。まだ実行せず、私が取り込みモードを選んだ後に取り込んで。", en: "Preview ~/Downloads/bookmarks.html before importing it into Codex URL Vault. Show the added, duplicate, and error counts, but do not apply it until I choose the import mode." },
  },
  {
    title: { ja: "既存URLを整理する", en: "Organize an existing URL" },
    route: "get_url → update_url / move_url",
    prompt: { ja: "Codex URL Vaultでエイリアス「example」の保存内容を確認してから、「Research/Web」カテゴリへ移動し、既存タグを残したまま「sample」を追加して。完了後に更新結果を見せて。", en: "In Codex URL Vault, inspect the saved URL with alias “example,” move it to “Research/Web,” and add the tag “sample” without removing its existing tags. Show the updated result." },
  },
];
const EXPORT_INSTRUCTIONS = {
  ja: {
    brave: "メニュー → ブックマーク → ブックマーク マネージャー → 右上のメニュー → ブックマークをエクスポート",
    chrome: "右上のメニュー → ブックマークとリスト → ブックマーク マネージャー → 右上のメニュー → ブックマークをエクスポート",
    firefox: "メニュー → ブックマーク → ブックマークを管理 → インポートとバックアップ → HTMLとしてエクスポート",
    safari: "ファイル → 書き出す → ブックマーク",
    other: "ブックマーク管理画面で「エクスポート」または「HTMLとして書き出す」を選びます。",
  },
  en: {
    brave: "Menu → Bookmarks → Bookmark Manager → More options → Export bookmarks",
    chrome: "More → Bookmarks and lists → Bookmark Manager → More → Export bookmarks",
    firefox: "Menu → Bookmarks → Manage bookmarks → Import and Backup → Export Bookmarks to HTML",
    safari: "File → Export → Bookmarks",
    other: "Open your browser's bookmark manager and choose Export or Export as HTML.",
  },
};
const BROWSER_NAMES = {
  brave: "Brave",
  chrome: "Chrome",
  firefox: "Firefox",
  safari: "Safari",
  other: "Other",
};

function normalize(value) {
  return String(value || "").toLowerCase();
}

function readPreferences() {
  let raw = "";
  try {
    raw = localStorage.getItem(PREFERENCES_KEY) || "";
  } catch {
    raw = sessionStorage.getItem(PREFERENCES_KEY) || "";
  }
  let stored = {};
  try {
    stored = JSON.parse(raw || "{}");
  } catch {
    stored = {};
  }
  let theme = THEMES.includes(stored.theme) ? stored.theme : "system";
  const language = LANGUAGES.includes(stored.language) ? stored.language : "system";
  if (!raw) {
    let legacyTheme = null;
    try {
      legacyTheme = localStorage.getItem(LEGACY_THEME_KEY);
    } catch {
      legacyTheme = null;
    }
    theme = legacyTheme === "auto" ? "system" : (THEMES.includes(legacyTheme) ? legacyTheme : "system");
  }
  return { language, theme };
}

function resolvedLanguage() {
  if (preferences.language !== "system") return preferences.language;
  const languages = navigator.languages?.length ? navigator.languages : [navigator.language];
  return languages.some((value) => String(value || "").toLowerCase().startsWith("ja")) ? "ja" : "en";
}

function t(key) {
  const language = resolvedLanguage();
  return HOST_MESSAGES[AGENT_HOST][language][key] || MESSAGES[language][key] || MESSAGES.en[key] || key;
}

function persistPreferences() {
  const value = JSON.stringify(preferences);
  try {
    localStorage.setItem(PREFERENCES_KEY, value);
  } catch {
    sessionStorage.setItem(PREFERENCES_KEY, value);
  }
}

function applyThemePreference() {
  if (preferences.theme === "system") {
    document.documentElement.removeAttribute("data-mlh-theme");
  } else {
    document.documentElement.setAttribute("data-mlh-theme", preferences.theme);
  }
  document.querySelectorAll("[data-theme-option]").forEach((button) => {
    const active = button.dataset.themeOption === preferences.theme;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
  });
}

function applyLanguagePreference() {
  const language = resolvedLanguage();
  document.documentElement.lang = language;
  document.querySelectorAll("[data-language-option]").forEach((button) => {
    const active = button.dataset.languageOption === preferences.language;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
  });
  document.querySelectorAll("[data-i18n]").forEach((node) => {
    node.textContent = t(node.dataset.i18n);
  });
  document.querySelectorAll("[data-i18n-placeholder]").forEach((node) => {
    node.setAttribute("placeholder", t(node.dataset.i18nPlaceholder));
  });
  document.querySelectorAll("[data-i18n-aria-label]").forEach((node) => {
    node.setAttribute("aria-label", t(node.dataset.i18nAriaLabel));
  });
  renderImportFlow();
}

function applyPreferences(options = {}) {
  persistPreferences();
  applyThemePreference();
  applyLanguagePreference();
  if (options.render !== false) {
    renderPromptSamples();
    render();
  }
}

function setPreference(kind, value) {
  const allowed = kind === "theme" ? THEMES : LANGUAGES;
  preferences[kind] = allowed.includes(value) ? value : "system";
  applyPreferences();
}

function compactFolder(folder) {
  if (!folder) return "Unfiled";
  return folder.replace(/^ブックマーク\/ブックマーク\/?/, "") || "Root";
}

function rawFolder(display) {
  if (!display || display === "All" || display === t("all") || display === "Unfiled" || display === t("unfiled")) return "";
  const category = categories.find((item) => item.display === display || item.path === display || compactFolder(item.path) === display);
  return category ? category.path : display;
}

function searchableText(item) {
  return [
    item.title,
    item.url,
    item.canonical_url,
    item.folder_path,
    item.note,
    item.project,
    item.snapshot?.preview,
    ...(item.tags || []),
    ...(item.aliases || []),
    ...(item.intents || []),
  ].map(normalize).join(" ");
}

function filteredItems() {
  const query = normalize(state.query).trim();
  return bookmarks.filter((item) => {
    const folderOk = state.folder === "All" || compactFolder(item.folder_path) === state.folder;
    const queryOk = !query || searchableText(item).includes(query);
    return folderOk && queryOk;
  });
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options.headers || {}),
    },
  });
  const payload = await response.json();
  if (!response.ok || payload.ok === false) {
    throw new Error(payload.error || `HTTP ${response.status}`);
  }
  return payload;
}

async function copyText(text) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  textarea.remove();
}

function showCopied(button) {
  const original = button.textContent;
  button.textContent = t("copied");
  button.disabled = true;
  setTimeout(() => {
    button.textContent = original;
    button.disabled = false;
  }, 1200);
}

async function loadVault() {
  const payload = await api("/api/bookmarks");
  bookmarks = payload.bookmarks || [];
  categories = payload.categories || [];
  if (restoredFromSession && bookmarks.length && filteredItems().length === 0) {
    clearFilters({ persist: false });
  }
  render();
}

function renderPromptSamples() {
  promptSamplesList.innerHTML = "";
  const fragment = document.createDocumentFragment();
  const language = resolvedLanguage();
  for (const sample of PROMPT_SAMPLES) {
    const title = sample.title[language] || sample.title.en;
    const prompt = sample.prompt[language] || sample.prompt.en;
    const row = document.createElement("section");
    row.className = "prompt-sample";
    row.innerHTML = `
      <div class="prompt-sample-heading">
        <h3>${escapeHtml(title)}</h3>
        <code class="mcp-route">${escapeHtml(t("mcpRoute"))}: ${escapeHtml(sample.route)}</code>
      </div>
      <pre class="prompt-text">${escapeHtml(prompt)}</pre>
      <footer>
        <button class="action-button" type="button" data-copy-prompt="${escapeAttr(prompt)}">${escapeHtml(t("copy"))}</button>
      </footer>
    `;
    fragment.appendChild(row);
  }
  promptSamplesList.appendChild(fragment);
}

function detectedSourceBrowser() {
  const userAgent = navigator.userAgent || "";
  if (navigator.brave) return "brave";
  if (/Firefox\//i.test(userAgent)) return "firefox";
  if (/Safari\//i.test(userAgent) && !/Chrome\//i.test(userAgent)) return "safari";
  if (/Chrome\//i.test(userAgent)) return "chrome";
  return "";
}

function selectedSourceBrowser() {
  return document.querySelector('input[name="sourceBrowser"]:checked')?.value || "";
}

function selectedImportMode() {
  return document.querySelector('input[name="importMode"]:checked')?.value || "merge";
}

function formatFileSize(size) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function importErrorKey(code) {
  if (code === "bookmark_html_unrecognized" || code === "bookmark_html_invalid" || code === "bookmark_html_contains_nul") return "unrecognizedHtml";
  if (code === "bookmark_html_has_no_bookmarks") return "noBookmarksInHtml";
  if (code === "bookmark_html_too_large" || code === "request_too_large") return "fileTooLargeError";
  if (code === "preview_checksum_mismatch") return "previewChanged";
  if (code === "reset_confirmation_required") return "resetConfirmError";
  return "importFailed";
}

function setImportStatus(key = "", kind = "info") {
  importState.statusKey = key;
  importState.statusKind = kind;
  importStatus.hidden = !key;
  importStatus.className = `import-status ${kind}`;
  importStatus.textContent = key ? t(key) : "";
}

function renderImportFlow() {
  if (!importDialog) return;
  const sourceBrowser = selectedSourceBrowser();
  const language = resolvedLanguage();
  const guide = document.querySelector("#browserExportGuide");
  const instructions = document.querySelector("#browserExportInstructions");
  guide.hidden = !sourceBrowser;
  instructions.textContent = sourceBrowser ? (EXPORT_INSTRUCTIONS[language][sourceBrowser] || EXPORT_INSTRUCTIONS[language].other) : "";

  const finished = Boolean(importState.result);
  document.querySelector("#importStepper").hidden = finished;
  document.querySelectorAll("[data-import-step]").forEach((section) => {
    section.hidden = finished || Number(section.dataset.importStep) !== importState.step;
  });
  document.querySelectorAll("[data-step-indicator]").forEach((item) => {
    const step = Number(item.dataset.stepIndicator);
    item.classList.toggle("current", step === importState.step);
    item.classList.toggle("complete", step < importState.step);
    if (step === importState.step) item.setAttribute("aria-current", "step");
    else item.removeAttribute("aria-current");
  });

  selectedFileName.textContent = importState.file
    ? `${importState.file.name} · ${formatFileSize(importState.file.size)}`
    : t("htmlOnly");
  filePreview.hidden = !importState.preview;
  if (importState.preview) {
    document.querySelector("#previewBookmarkCount").textContent = importState.preview.unique_count.toLocaleString();
    document.querySelector("#previewFolderCount").textContent = importState.preview.folder_count.toLocaleString();
    document.querySelector("#previewDuplicateCount").textContent = importState.preview.duplicate_count.toLocaleString();
  }

  const mode = selectedImportMode();
  resetConfirmRow.hidden = mode !== "reset";
  if (mode !== "reset") resetConfirm.checked = false;
  document.querySelector("#reviewBrowser").textContent = sourceBrowser
    ? (sourceBrowser === "other" ? t("otherBrowser") : BROWSER_NAMES[sourceBrowser])
    : "-";
  document.querySelector("#reviewFile").textContent = importState.file?.name || "-";
  document.querySelector("#reviewCount").textContent = importState.preview
    ? importState.preview.unique_count.toLocaleString()
    : "-";

  importResult.hidden = !finished;
  if (finished) {
    const result = importState.result;
    document.querySelector("#importResultTitle").textContent = t("importedTitle").replace("{count}", result.unique_count.toLocaleString());
    document.querySelector("#importResultSummary").textContent = t(`${result.mode}Result`);
    document.querySelector("#resultCreated").textContent = result.created.toLocaleString();
    document.querySelector("#resultUpdated").textContent = result.updated.toLocaleString();
    document.querySelector("#resultSkipped").textContent = result.skipped.toLocaleString();
  }

  importBackButton.hidden = finished || importState.step === 1;
  importNextButton.hidden = finished || importState.step === 3;
  importRunButton.hidden = finished || importState.step !== 3;
  importDoneButton.hidden = !finished;
  importCancelButton.hidden = finished;
  importBackButton.disabled = importState.busy;
  importNextButton.disabled = importState.busy || (importState.step === 1 ? !sourceBrowser : !importState.preview);
  importRunButton.disabled = importState.busy || !importState.preview || (mode === "reset" && !resetConfirm.checked);
  importCancelButton.disabled = importState.busy;
  document.querySelector("#importDialogClose").disabled = importState.busy;
  importFile.disabled = importState.busy;
  stripCommonRoot.disabled = importState.busy;
  document.querySelectorAll('input[name="sourceBrowser"], input[name="importMode"], #sourceProfile, #resetConfirm').forEach((control) => {
    control.disabled = importState.busy;
  });
  if (importState.statusKey) setImportStatus(importState.statusKey, importState.statusKind);
}

function resetImportFlow() {
  importForm.reset();
  importFile.value = "";
  Object.assign(importState, {
    step: 1,
    file: null,
    text: "",
    preview: null,
    result: null,
    busy: false,
    statusKey: "",
    statusKind: "info",
  });
  const detected = detectedSourceBrowser();
  const browserInput = detected ? document.querySelector(`input[name="sourceBrowser"][value="${detected}"]`) : null;
  if (browserInput) browserInput.checked = true;
  setImportStatus();
  renderImportFlow();
}

function openImportFlow() {
  resetImportFlow();
  importDialog.showModal();
  const firstControl = document.querySelector('input[name="sourceBrowser"]:checked') || document.querySelector('input[name="sourceBrowser"]');
  firstControl?.focus();
}

function closeImportFlow() {
  if (importState.busy) return;
  importDialog.close();
}

async function previewImportFile() {
  if (!importState.file || !importState.text) return;
  const textAtRequest = importState.text;
  const stripAtRequest = stripCommonRoot.checked;
  importState.preview = null;
  importState.busy = true;
  setImportStatus("analyzingFile", "loading");
  renderImportFlow();
  try {
    const payload = await api("/api/import/preview", {
      method: "POST",
      body: JSON.stringify({
        file_name: importState.file.name,
        content: textAtRequest,
        strip_common_root: stripAtRequest,
      }),
    });
    if (textAtRequest !== importState.text || stripAtRequest !== stripCommonRoot.checked) return;
    importState.preview = payload.preview;
    setImportStatus("importReady", "success");
  } catch (error) {
    if (textAtRequest === importState.text) setImportStatus(importErrorKey(error.message), "error");
  } finally {
    if (textAtRequest === importState.text) importState.busy = false;
    renderImportFlow();
  }
}

async function selectImportFile(file) {
  importState.file = null;
  importState.text = "";
  importState.preview = null;
  if (!file) {
    setImportStatus("chooseFileError", "error");
    renderImportFlow();
    return;
  }
  if (!/\.html?$/i.test(file.name)) {
    setImportStatus("invalidFileError", "error");
    importFile.value = "";
    renderImportFlow();
    return;
  }
  if (file.size > MAX_IMPORT_FILE_BYTES) {
    setImportStatus("fileTooLargeError", "error");
    importFile.value = "";
    renderImportFlow();
    return;
  }
  importState.file = file;
  importState.text = await file.text();
  await previewImportFile();
}

function advanceImportFlow() {
  if (importState.step === 1 && !selectedSourceBrowser()) {
    setImportStatus("chooseBrowserError", "error");
    return;
  }
  if (importState.step === 2 && !importState.preview) {
    setImportStatus("chooseFileError", "error");
    return;
  }
  setImportStatus();
  importState.step = Math.min(3, importState.step + 1);
  renderImportFlow();
}

function retreatImportFlow() {
  if (importState.busy) return;
  setImportStatus();
  importState.step = Math.max(1, importState.step - 1);
  renderImportFlow();
}

async function runBookmarkImport(event) {
  event.preventDefault();
  if (!importState.preview || !importState.file) {
    setImportStatus("chooseFileError", "error");
    return;
  }
  const mode = selectedImportMode();
  if (mode === "reset" && !resetConfirm.checked) {
    setImportStatus("resetConfirmError", "error");
    renderImportFlow();
    return;
  }
  importState.busy = true;
  setImportStatus("importingFile", "loading");
  renderImportFlow();
  try {
    const sourceBrowser = selectedSourceBrowser();
    const payload = await api("/api/import", {
      method: "POST",
      body: JSON.stringify({
        file_name: importState.file.name,
        content: importState.text,
        source_browser: sourceBrowser,
        source_profile: document.querySelector("#sourceProfile").value.trim(),
        preferred_browser: sourceBrowser === "other" ? null : sourceBrowser,
        mode,
        strip_common_root: stripCommonRoot.checked,
        expected_sha256: importState.preview.content_sha256,
        confirm_reset: mode === "reset" && resetConfirm.checked,
      }),
    });
    importState.result = payload.result;
    setImportStatus();
    state.folder = "All";
    state.query = "";
    searchInput.value = "";
    sessionStorage.removeItem(STATE_KEY);
    await loadVault();
  } catch (error) {
    setImportStatus(importErrorKey(error.message), "error");
  } finally {
    importState.busy = false;
    renderImportFlow();
    if (importState.result) importResult.focus();
  }
}

function renderFolders() {
  folderList.innerHTML = "";
  const unfiled = {
    display: "Unfiled",
    count: bookmarks.filter((item) => !item.folder_path).length,
    path: "",
  };
  const rows = [
    { display: "All", count: bookmarks.length, path: "All" },
    unfiled,
    ...categories.filter((category) => category.path),
  ];
  for (const category of rows) {
    const folder = category.path === "All" ? "All" : compactFolder(category.path);
    const canManage = folder !== "All" && folder !== "Unfiled";
    const folderLabel = folder === "All" ? t("all") : (folder === "Unfiled" ? t("unfiled") : folder);
    const button = document.createElement("button");
    button.className = `folder-button${state.folder === folder ? " active" : ""}`;
    button.type = "button";
    button.innerHTML = `
      <span class="folder-name">${escapeHtml(folderLabel)}</span>
      <span class="folder-tools">
        <span class="folder-count">${category.count ?? 0}</span>
        ${canManage ? `<span class="folder-menu" aria-hidden="true">...</span>` : ""}
      </span>
    `;
    button.addEventListener("click", () => {
      state.folder = folder;
      render();
    });
    if (canManage) {
      button.addEventListener("contextmenu", async (event) => {
        event.preventDefault();
        await editCategory(category.path);
      });
      button.querySelector(".folder-menu").addEventListener("click", async (event) => {
        event.preventDefault();
        event.stopPropagation();
        await editCategory(category.path);
      });
    }
    folderList.appendChild(button);
  }
}

function renderCards(items) {
  grid.innerHTML = "";
  const fragment = document.createDocumentFragment();
  for (const item of items) {
    const card = document.createElement("article");
    card.className = "card";
    const chips = [
      ...(item.aliases || []).map((label) => ({ label, kind: "alias" })),
      ...(item.tags || []).map((label) => ({ label, kind: "" })),
      ...(item.intents || []).map((label) => ({ label, kind: "" })),
    ].slice(0, 6);
    const targetUrl = item.url || item.canonical_url;
    const targetTitle = item.title || targetUrl;
    const snapshot = item.snapshot;
    const readerHref = readerUrl(targetUrl, targetTitle);
    const launchHref = launchUrl(targetUrl, targetTitle);
    const rememberState = () => {
      sessionStorage.setItem(STATE_KEY, JSON.stringify(state));
    };
    const openReader = () => {
      if (!targetUrl) return;
      rememberState();
      window.location.assign(readerHref);
    };
    const openDirect = () => {
      if (!targetUrl) return;
      rememberState();
      window.location.assign(launchHref);
    };
    const openExternal = async () => {
      if (!targetUrl) return;
      rememberState();
      await api(`/open-external?url=${encodeURIComponent(targetUrl)}`, { headers: {} });
    };
    card.innerHTML = `
      <a class="card-link" href="${escapeAttr(launchHref)}" rel="noreferrer" title="${escapeAttr(t("iab"))}" aria-label="${escapeAttr(`${t("openItem")} ${targetTitle}`)}">
        <div class="card-title">${escapeHtml(targetTitle)}</div>
        <div class="url">${escapeHtml(targetUrl)}</div>
      </a>
      <div class="folder">${escapeHtml(compactFolder(item.folder_path) === "Unfiled" ? t("unfiled") : compactFolder(item.folder_path))}</div>
      ${snapshot ? `<div class="snapshot-badge ${escapeAttr(snapshot.status)}">${escapeHtml(t("savedCopy"))} · ${escapeHtml(snapshot.kind || "-")} · ${escapeHtml(snapshot.status || "-")}</div>` : ""}
      ${chips.length ? `<div class="chips">${chips.map((chip) => `<span class="chip ${chip.kind}">${escapeHtml(chip.label)}</span>`).join("")}</div>` : ""}
      <div class="card-actions">
        ${snapshot ? `<button class="action-button primary" type="button" data-action="snapshot">${escapeHtml(t("savedCopy"))}</button>` : ""}
        <button class="action-button${snapshot ? "" : " primary"}" type="button" data-action="iab">${escapeHtml(t("iab"))}</button>
        <button class="action-button" type="button" data-action="preview">${escapeHtml(t("preview"))}</button>
        <button class="action-button" type="button" data-action="external">${escapeHtml(t("defaultBrowser"))}</button>
        <button class="action-button" type="button" data-action="edit">${escapeHtml(t("edit"))}</button>
      </div>
    `;
    const link = card.querySelector(".card-link");
    link.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      openDirect();
    });
    card.querySelector(".card-actions").addEventListener("click", async (event) => {
      const button = event.target.closest("button");
      if (!button) return;
      event.stopPropagation();
      if (button.dataset.action === "snapshot") await openSnapshot(item);
      if (button.dataset.action === "iab") openDirect();
      if (button.dataset.action === "preview") openReader();
      if (button.dataset.action === "external") await openExternal();
      if (button.dataset.action === "edit") openDialog(item);
    });
    card.addEventListener("click", (event) => {
      if (event.target.closest("a, button")) return;
      openDirect();
    });
    card.tabIndex = 0;
    card.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      openDirect();
    });
    fragment.appendChild(card);
  }
  grid.appendChild(fragment);
}

function vaultUrl() {
  const url = new URL(location.href);
  url.hash = "";
  return url.toString();
}

async function openVaultExternal() {
  await api(`/open-external?url=${encodeURIComponent(vaultUrl())}`, { headers: {} });
}

async function openSnapshot(item) {
  const payload = await api(`/api/bookmarks/${encodeURIComponent(item.id)}/snapshot`, { headers: {} });
  const snapshot = payload.snapshot;
  snapshotTitle.textContent = snapshot.title || payload.bookmark.title || payload.bookmark.url;
  snapshotMeta.textContent = [
    `${t("snapshotKind")}: ${snapshot.kind || "-"}`,
    `${t("snapshotStatus")}: ${snapshot.status || "-"}`,
    `${t("capturedAt")}: ${snapshot.captured_at || "-"}`,
  ].join(" · ");
  snapshotContent.textContent = snapshot.content ?? `${t("snapshotUnavailable")} (${snapshot.status || "unknown"})`;
  snapshotContent.classList.toggle("invalid", snapshot.status !== "valid");
  snapshotDialog.showModal();
}

function render() {
  const items = filteredItems();
  const showOnboarding = bookmarks.length === 0 && state.folder === "All" && !state.query;
  visibleCount.textContent = items.length.toLocaleString();
  totalCount.textContent = bookmarks.length.toLocaleString();
  viewTitle.textContent = state.folder === "All" ? t("allBookmarks") : (state.folder === "Unfiled" ? t("unfiled") : state.folder);
  importOnboarding.hidden = !showOnboarding;
  emptyState.hidden = items.length !== 0 || showOnboarding;
  clearFiltersButton.hidden = state.folder === "All" && !state.query;
  renderCategoryOptions();
  renderFolders();
  renderCards(items);
}

function clearFilters(options = {}) {
  state.folder = "All";
  state.query = "";
  searchInput.value = "";
  if (options.persist !== false) {
    sessionStorage.removeItem(STATE_KEY);
    render();
  }
}

function renderCategoryOptions() {
  const datalist = document.querySelector("#categoryOptions");
  datalist.innerHTML = "";
  const unfiledOption = document.createElement("option");
  unfiledOption.value = t("unfiled");
  unfiledOption.label = t("unfiled");
  datalist.appendChild(unfiledOption);
  for (const category of categories) {
    if (!category.path) continue;
    const option = document.createElement("option");
    option.value = category.path || "";
    option.label = category.display || category.path || "Unfiled";
    datalist.appendChild(option);
  }
}

function openDialog(item = null) {
  document.querySelector("#dialogTitle").textContent = item ? t("editUrl") : t("addUrl");
  document.querySelector("#bookmarkId").value = item?.id || "";
  document.querySelector("#bookmarkUrl").value = item?.url || "";
  document.querySelector("#bookmarkTitle").value = item?.title || "";
  document.querySelector("#bookmarkCategory").value = item ? (item.folder_path || t("unfiled")) : t("unfiled");
  document.querySelector("#bookmarkAliases").value = (item?.aliases || []).join(", ");
  document.querySelector("#bookmarkTags").value = (item?.tags || []).join(", ");
  document.querySelector("#bookmarkIntents").value = (item?.intents || []).join(", ");
  document.querySelector("#bookmarkNote").value = item?.note || "";
  deleteBookmarkButton.hidden = !item;
  dialog.showModal();
}

function dialogPayload() {
  return {
    url: document.querySelector("#bookmarkUrl").value.trim(),
    title: document.querySelector("#bookmarkTitle").value.trim(),
    category: rawFolder(document.querySelector("#bookmarkCategory").value.trim()),
    aliases: document.querySelector("#bookmarkAliases").value.trim(),
    tags: document.querySelector("#bookmarkTags").value.trim(),
    intents: document.querySelector("#bookmarkIntents").value.trim(),
    note: document.querySelector("#bookmarkNote").value.trim(),
    source_type: `${AGENT_HOST}_capture`,
  };
}

async function saveDialog(event) {
  event.preventDefault();
  const id = document.querySelector("#bookmarkId").value;
  const payload = dialogPayload();
  if (id) {
    await api(`/api/bookmarks/${encodeURIComponent(id)}`, {
      method: "PATCH",
      body: JSON.stringify(payload),
    });
  } else {
    await api("/api/bookmarks", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }
  dialog.close();
  await loadVault();
}

async function deleteCurrentBookmark() {
  const id = document.querySelector("#bookmarkId").value;
  if (!id) return;
  if (!confirm(t("deleteConfirm"))) return;
  await api(`/api/bookmarks/${encodeURIComponent(id)}`, { method: "DELETE" });
  dialog.close();
  await loadVault();
}

function openCategoryDialog(path = null) {
  editingCategoryPath = typeof path === "string" ? path : null;
  categoryForm.reset();
  categoryPath.value = editingCategoryPath || "";
  const titleKey = editingCategoryPath === null ? "addCategory" : "editCategory";
  categoryDialogTitle.dataset.i18n = titleKey;
  categoryDialogTitle.textContent = t(titleKey);
  deleteCategoryButton.hidden = editingCategoryPath === null;
  categoryDeleteConfirm.hidden = true;
  categoryDialog.showModal();
  categoryPath.focus();
}

async function saveCategory(event) {
  event.preventDefault();
  const path = categoryPath.value.trim();
  if (!path) {
    categoryPath.focus();
    return;
  }
  if (editingCategoryPath === null) {
    await api("/api/categories", {
      method: "POST",
      body: JSON.stringify({ path }),
    });
  } else if (path !== editingCategoryPath) {
    await api("/api/categories", {
      method: "PATCH",
      body: JSON.stringify({ old_path: editingCategoryPath, new_path: path }),
    });
    state.folder = compactFolder(path);
  }
  categoryDialog.close();
  await loadVault();
}

function editCategory(path) {
  openCategoryDialog(path);
}

function showCategoryDeleteConfirmation() {
  if (editingCategoryPath === null) return;
  categoryDeleteConfirm.hidden = false;
  confirmCategoryDeleteButton.focus();
}

async function deleteCategory() {
  if (editingCategoryPath === null) return;
  await api(`/api/categories?path=${encodeURIComponent(editingCategoryPath)}&move_to=`, {
    method: "DELETE",
  });
  categoryDialog.close();
  state.folder = "All";
  await loadVault();
}

function escapeHtml(value) {
  return String(value || "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function escapeAttr(value) {
  return escapeHtml(value).replaceAll("`", "&#096;");
}

function readerUrl(targetUrl, title) {
  const params = new URLSearchParams({ url: targetUrl, title });
  return `./open.html?${params.toString()}`;
}

function launchUrl(targetUrl, title) {
  const rid = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  const params = new URLSearchParams({ url: targetUrl, title, rid });
  return `./go.html?${params.toString()}`;
}

function restoreState() {
  const raw = sessionStorage.getItem(STATE_KEY);
  if (!raw) return;
  try {
    const restored = JSON.parse(raw);
    state.folder = restored.folder || "All";
    state.query = restored.query || "";
    searchInput.value = state.query;
    restoredFromSession = true;
  } catch {
    sessionStorage.removeItem(STATE_KEY);
  }
}

function setSidebarWidth(width, { persist = false } = {}) {
  const nextWidth = Math.max(220, Math.min(520, Math.round(Number(width) || 320)));
  document.documentElement.style.setProperty("--sidebar-width", `${nextWidth}px`);
  sidebarResizeHandle.setAttribute("aria-valuenow", String(nextWidth));
  if (persist) localStorage.setItem(SIDEBAR_WIDTH_KEY, String(nextWidth));
}

function restoreSidebarWidth() {
  setSidebarWidth(localStorage.getItem(SIDEBAR_WIDTH_KEY) || 320);
}

sidebarResizeHandle.addEventListener("pointerdown", (event) => {
  if (window.matchMedia("(max-width: 900px)").matches) return;
  sidebarResizeHandle.classList.add("dragging");
  sidebarResizeHandle.setPointerCapture(event.pointerId);
  setSidebarWidth(event.clientX);
});

sidebarResizeHandle.addEventListener("pointermove", (event) => {
  if (sidebarResizeHandle.hasPointerCapture(event.pointerId)) {
    setSidebarWidth(event.clientX);
  }
});

function finishSidebarResize(event) {
  if (!sidebarResizeHandle.hasPointerCapture(event.pointerId)) return;
  sidebarResizeHandle.releasePointerCapture(event.pointerId);
  sidebarResizeHandle.classList.remove("dragging");
  setSidebarWidth(event.clientX, { persist: true });
}

sidebarResizeHandle.addEventListener("pointerup", finishSidebarResize);
sidebarResizeHandle.addEventListener("pointercancel", finishSidebarResize);
sidebarResizeHandle.addEventListener("keydown", (event) => {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  const current = Number(sidebarResizeHandle.getAttribute("aria-valuenow")) || 320;
  const next = event.key === "Home"
    ? 220
    : event.key === "End"
      ? 520
      : current + (event.key === "ArrowLeft" ? -20 : 20);
  setSidebarWidth(next, { persist: true });
});

searchInput.addEventListener("input", (event) => {
  state.query = event.target.value;
  render();
});

document.querySelectorAll("[data-theme-option]").forEach((button) => {
  button.addEventListener("click", () => setPreference("theme", button.dataset.themeOption));
});
document.querySelectorAll("[data-language-option]").forEach((button) => {
  button.addEventListener("click", () => setPreference("language", button.dataset.languageOption));
});
openVaultExternalButton.addEventListener("click", openVaultExternal);
promptSamplesButton.addEventListener("click", () => promptSamplesDialog.showModal());
importBookmarksButton.addEventListener("click", openImportFlow);
onboardingImportButton.addEventListener("click", openImportFlow);
document.querySelector("#importDialogClose").addEventListener("click", closeImportFlow);
importCancelButton.addEventListener("click", closeImportFlow);
importDoneButton.addEventListener("click", closeImportFlow);
importNextButton.addEventListener("click", advanceImportFlow);
importBackButton.addEventListener("click", retreatImportFlow);
importForm.addEventListener("submit", runBookmarkImport);
importDialog.addEventListener("cancel", (event) => {
  if (importState.busy) event.preventDefault();
});
document.querySelectorAll('input[name="sourceBrowser"]').forEach((input) => {
  input.addEventListener("change", () => {
    setImportStatus();
    renderImportFlow();
  });
});
document.querySelectorAll('input[name="importMode"]').forEach((input) => {
  input.addEventListener("change", renderImportFlow);
});
resetConfirm.addEventListener("change", renderImportFlow);
importFile.addEventListener("change", () => selectImportFile(importFile.files?.[0]));
stripCommonRoot.addEventListener("change", () => {
  if (importState.file) previewImportFile();
});
["dragenter", "dragover"].forEach((eventName) => {
  importDropZone.addEventListener(eventName, (event) => {
    event.preventDefault();
    if (!importState.busy) importDropZone.classList.add("drag-active");
  });
});
["dragleave", "drop"].forEach((eventName) => {
  importDropZone.addEventListener(eventName, (event) => {
    event.preventDefault();
    importDropZone.classList.remove("drag-active");
  });
});
importDropZone.addEventListener("drop", (event) => {
  if (!importState.busy) selectImportFile(event.dataTransfer?.files?.[0]);
});
promptSamplesList.addEventListener("click", async (event) => {
  const button = event.target.closest("[data-copy-prompt]");
  if (!button) return;
  await copyText(button.dataset.copyPrompt);
  showCopied(button);
});
copyAllPromptsButton.addEventListener("click", async () => {
  const language = resolvedLanguage();
  await copyText(PROMPT_SAMPLES.map((sample) => sample.prompt[language] || sample.prompt.en).join("\n"));
  showCopied(copyAllPromptsButton);
});
addUrlButton.addEventListener("click", () => openDialog());
addCategoryButton.addEventListener("click", openCategoryDialog);
clearFiltersButton.addEventListener("click", clearFilters);
form.addEventListener("submit", saveDialog);
categoryForm.addEventListener("submit", saveCategory);
deleteBookmarkButton.addEventListener("click", deleteCurrentBookmark);
document.querySelector("#dialogClose").addEventListener("click", () => dialog.close());
document.querySelector("#cancelBookmarkButton").addEventListener("click", () => dialog.close());
document.querySelector("#categoryDialogClose").addEventListener("click", () => categoryDialog.close());
document.querySelector("#cancelCategoryButton").addEventListener("click", () => categoryDialog.close());
deleteCategoryButton.addEventListener("click", showCategoryDeleteConfirmation);
document.querySelector("#cancelCategoryDeleteButton").addEventListener("click", () => {
  categoryDeleteConfirm.hidden = true;
  categoryPath.focus();
});
confirmCategoryDeleteButton.addEventListener("click", deleteCategory);
categoryDialog.addEventListener("close", () => {
  editingCategoryPath = null;
  categoryDeleteConfirm.hidden = true;
});
document.querySelector("#promptSamplesClose").addEventListener("click", () => promptSamplesDialog.close());
document.querySelector("#snapshotDialogClose").addEventListener("click", () => snapshotDialog.close());
document.querySelector("#snapshotCloseButton").addEventListener("click", () => snapshotDialog.close());
window.addEventListener("languagechange", () => {
  if (preferences.language === "system") applyPreferences();
});
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  if (preferences.theme === "system") applyThemePreference();
});

applyPreferences({ render: false });
renderPromptSamples();
restoreSidebarWidth();
restoreState();
if (location.search.includes("session=")) {
  const cleanUrl = new URL(location.href);
  cleanUrl.searchParams.delete("session");
  history.replaceState(null, "", `${cleanUrl.pathname}${cleanUrl.search}${cleanUrl.hash}`);
}
loadVault().catch((error) => {
  emptyState.hidden = false;
  emptyState.querySelector("h3").textContent = t("vaultUnavailable");
  emptyState.querySelector("p").textContent = error.message;
});
