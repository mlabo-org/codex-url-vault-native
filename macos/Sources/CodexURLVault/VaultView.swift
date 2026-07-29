import SwiftUI
import UniformTypeIdentifiers

struct VaultView: View {
    @Environment(\.appShellLanguage) private var language
    @StateObject private var store = VaultStore()
    @State private var showingAdd = false
    @State private var showingImport = false

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    var body: some View {
        NavigationSplitView {
            List {
                Button {
                    store.selectCategory(nil)
                } label: {
                    Label(strings.allURLs, systemImage: "tray.full")
                }
                .buttonStyle(.plain)

                Section(strings.categories) {
                    ForEach(store.categories) { category in
                        Button {
                            store.selectCategory(category.path)
                        } label: {
                            HStack {
                                Label(
                                    category.display,
                                    systemImage: store.selectedCategory == category.path
                                        ? "folder.fill" : "folder"
                                )
                                Spacer()
                                Text("\(category.count)")
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .navigationTitle(strings.vault)
            .navigationSplitViewColumnWidth(min: 190, ideal: 230)
        } content: {
            List(store.bookmarks, selection: $store.selectedBookmarkID) { bookmark in
                BookmarkRow(bookmark: bookmark)
                    .tag(bookmark.id)
                    .contextMenu {
                        Button(strings.open) { store.open(bookmark) }
                        Divider()
                        Button(strings.archive, role: .destructive) {
                            store.archive(bookmark)
                        }
                    }
            }
            .navigationTitle(
                store.selectedCategory.flatMap { $0.isEmpty ? nil : $0 } ?? strings.allURLs
            )
            .searchable(text: $store.searchText, prompt: strings.searchPrompt)
            .onSubmit(of: .search) { store.search() }
            .onChange(of: store.searchText) { _, value in
                if value.isEmpty {
                    store.search()
                }
            }
        } detail: {
            if let bookmark = store.selectedBookmark {
                BookmarkDetail(bookmark: bookmark, store: store)
                    .id(bookmark.id + bookmark.updatedAt)
            } else {
                ContentUnavailableView(
                    strings.noSelection,
                    systemImage: "bookmark",
                    description: Text(strings.noSelectionHint)
                )
            }
        }
        .frame(minWidth: 960, minHeight: 620)
        .toolbar {
            ToolbarItemGroup {
                Button {
                    showingImport = true
                } label: {
                    Label(strings.importBookmarks, systemImage: "square.and.arrow.down")
                }
                Button {
                    showingAdd = true
                } label: {
                    Label(strings.addURL, systemImage: "plus")
                }
            }
        }
        .sheet(isPresented: $showingAdd) {
            AddBookmarkView(store: store)
        }
        .fileImporter(
            isPresented: $showingImport,
            allowedContentTypes: [.html],
            allowsMultipleSelection: false
        ) { result in
            switch result {
            case .success(let files):
                if let file = files.first {
                    store.importBookmarks(from: file)
                }
            case .failure(let error):
                store.errorMessage = error.localizedDescription
            }
        }
        .alert(
            strings.error,
            isPresented: Binding(
                get: { store.errorMessage != nil },
                set: { if !$0 { store.errorMessage = nil } }
            )
        ) {
            Button(strings.ok, role: .cancel) {}
        } message: {
            Text(store.errorMessage ?? "")
        }
        .overlay(alignment: .bottom) {
            if let message = store.activityMessage {
                Text(message)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(.regularMaterial, in: Capsule())
                    .padding()
                    .onTapGesture { store.activityMessage = nil }
            }
        }
        .task { store.start() }
    }
}

private struct BookmarkRow: View {
    let bookmark: VaultBookmark

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(bookmark.displayTitle)
                    .lineLimit(1)
                if bookmark.snapshot?.status == "valid" {
                    Image(systemName: "doc.text.fill")
                        .foregroundStyle(.secondary)
                        .accessibilityLabel("Snapshot")
                }
            }
            Text(bookmark.url)
                .appFont(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            if !bookmark.tags.isEmpty {
                Text(bookmark.tags.prefix(4).map { "#\($0)" }.joined(separator: "  "))
                    .appFont(.caption)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }
        }
        .padding(.vertical, 3)
    }
}

private struct BookmarkDetail: View {
    @Environment(\.appShellLanguage) private var language
    let bookmark: VaultBookmark
    @ObservedObject var store: VaultStore
    @State private var editing = false
    @State private var title = ""
    @State private var note = ""
    @State private var category = ""
    @State private var tags = ""
    @State private var fullSnapshot: VaultSnapshot?

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(bookmark.displayTitle)
                            .appFont(.title2)
                            .textSelection(.enabled)
                        Text(bookmark.url)
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                    Spacer()
                    Button(strings.open) { store.open(bookmark) }
                        .buttonStyle(.borderedProminent)
                }

                if editing {
                    Form {
                        TextField(strings.title, text: $title)
                        TextField(strings.category, text: $category)
                        TextField(strings.tags, text: $tags)
                        TextField(strings.note, text: $note, axis: .vertical)
                            .lineLimit(3...8)
                    }
                    .formStyle(.grouped)
                } else {
                    DetailGrid(bookmark: bookmark, strings: strings)
                }

                if let snapshot = fullSnapshot ?? bookmark.snapshot {
                    GroupBox(strings.snapshot) {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text(snapshot.status.capitalized)
                                Spacer()
                                Text(snapshot.kind)
                                    .foregroundStyle(.secondary)
                            }
                            if let content = snapshot.content ?? snapshot.preview {
                                Text(content)
                                    .appFont(.body)
                                    .textSelection(.enabled)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                        .padding(4)
                    }
                }

                HStack {
                    Button(editing ? strings.save : strings.edit) {
                        if editing {
                            if store.update(
                                bookmark: bookmark,
                                title: title,
                                note: note,
                                category: category,
                                tags: tags
                            ) {
                                editing = false
                            }
                        } else {
                            beginEditing()
                        }
                    }
                    Button(strings.archive, role: .destructive) {
                        store.archive(bookmark)
                    }
                }
            }
            .padding(24)
        }
        .navigationTitle(bookmark.displayTitle)
        .task {
            if bookmark.snapshot != nil {
                fullSnapshot = store.snapshot(for: bookmark)
            }
        }
    }

    private func beginEditing() {
        title = bookmark.title ?? ""
        note = bookmark.note ?? ""
        category = bookmark.folderPath ?? ""
        tags = bookmark.tags.joined(separator: ", ")
        editing = true
    }
}

private struct DetailGrid: View {
    let bookmark: VaultBookmark
    let strings: VaultStrings

    var body: some View {
        Grid(alignment: .leading, horizontalSpacing: 18, verticalSpacing: 10) {
            row(strings.category, bookmark.folderPath ?? strings.uncategorized)
            row(strings.tags, bookmark.tags.isEmpty ? "—" : bookmark.tags.joined(separator: ", "))
            row(strings.project, bookmark.project ?? "—")
            row(strings.opened, "\(bookmark.openCount)")
            if let note = bookmark.note, !note.isEmpty {
                row(strings.note, note)
            }
        }
    }

    @ViewBuilder
    private func row(_ label: String, _ value: String) -> some View {
        GridRow {
            Text(label)
                .foregroundStyle(.secondary)
            Text(value)
                .textSelection(.enabled)
        }
    }
}

private struct AddBookmarkView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.appShellLanguage) private var language
    @ObservedObject var store: VaultStore
    @State private var url = ""
    @State private var title = ""
    @State private var note = ""
    @State private var category = ""
    @State private var tags = ""

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(strings.addURL)
                .appFont(.title2)
            Form {
                TextField("URL", text: $url)
                TextField(strings.title, text: $title)
                TextField(strings.category, text: $category)
                TextField(strings.tags, text: $tags)
                TextField(strings.note, text: $note, axis: .vertical)
                    .lineLimit(3...8)
            }
            .formStyle(.grouped)
            HStack {
                Spacer()
                Button(strings.cancel) { dismiss() }
                Button(strings.save) {
                    if store.add(
                        url: url,
                        title: title,
                        note: note,
                        category: category,
                        tags: tags
                    ) {
                        dismiss()
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(url.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(24)
        .frame(width: 520)
    }
}

struct VaultStrings {
    private let shell: AppShellStrings

    init(language: AppShellLanguage) {
        shell = AppShellStrings(language: language)
    }

    private func text(_ japanese: String, _ english: String) -> String {
        shell.localized(japanese: japanese, english: english)
    }

    var vault: String { "Codex URL Vault" }
    var allURLs: String { text("すべてのURL", "All URLs") }
    var categories: String { text("カテゴリー", "Categories") }
    var category: String { text("カテゴリー", "Category") }
    var uncategorized: String { text("未分類", "Uncategorized") }
    var searchPrompt: String { text("URLを検索", "Search URLs") }
    var addURL: String { text("URLを追加", "Add URL") }
    var importBookmarks: String { text("ブックマークを読み込む", "Import Bookmarks") }
    var noSelection: String { text("URLが選択されていません", "No URL Selected") }
    var noSelectionHint: String { text("一覧からURLを選択してください。", "Choose a URL from the list.") }
    var open: String { text("開く", "Open") }
    var archive: String { text("アーカイブ", "Archive") }
    var title: String { text("タイトル", "Title") }
    var tags: String { text("タグ", "Tags") }
    var note: String { text("メモ", "Note") }
    var project: String { text("プロジェクト", "Project") }
    var opened: String { text("開いた回数", "Open count") }
    var snapshot: String { text("保存テキスト", "Snapshot") }
    var edit: String { text("編集", "Edit") }
    var save: String { text("保存", "Save") }
    var cancel: String { text("キャンセル", "Cancel") }
    var error: String { text("エラー", "Error") }
    var ok: String { "OK" }
}
