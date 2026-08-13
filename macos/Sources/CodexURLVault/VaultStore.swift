import Combine
import Foundation

struct PreparedVaultImport {
    let html: String
    let fileName: String
    let preview: VaultImportPreview
}

@MainActor
final class VaultStore: ObservableObject {
    @Published private(set) var bookmarks: [VaultBookmark] = []
    @Published private(set) var categories: [VaultCategory] = []
    @Published private(set) var totalCount = 0
    @Published var selectedCategory: String?
    @Published var searchText = ""
    @Published var errorMessage: String?
    @Published var activityMessage: String?

    private var dataChangeMonitor: VaultDataChangeMonitor?

    func start() {
        guard dataChangeMonitor == nil else { return }
        do {
            let initialization = try RustVaultBridge.initialize()
            try reload()
            let canonicalHTMLURL = URL(fileURLWithPath: initialization.db)
                .deletingLastPathComponent()
                .appendingPathComponent("current-bookmarks.html")
            let monitor = VaultDataChangeMonitor(canonicalHTMLURL: canonicalHTMLURL) { [weak self] in
                Task { @MainActor [weak self] in
                    self?.reloadAfterExternalChange()
                }
            }
            try monitor.start()
            dataChangeMonitor = monitor
        } catch {
            fail(error)
        }
    }

    func reload() throws {
        categories = try RustVaultBridge.listCategories()
        let allBookmarks = try RustVaultBridge.listURLs(category: nil)
        totalCount = allBookmarks.count
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        if query.isEmpty {
            bookmarks = selectedCategory == nil
                ? allBookmarks
                : allBookmarks.filter { ($0.folderPath ?? "") == selectedCategory }
        } else {
            let matches = try RustVaultBridge.searchURLs(query: query, limit: 10_000)
            bookmarks = selectedCategory == nil
                ? matches
                : matches.filter { ($0.folderPath ?? "") == selectedCategory }
        }
        dataChangeMonitor?.acknowledgeCurrentState()
    }

    func selectCategory(_ category: String?) {
        selectedCategory = category
        searchText = ""
        do {
            try reload()
        } catch {
            fail(error)
        }
    }

    func search() {
        do {
            let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
            if query.isEmpty {
                bookmarks = try RustVaultBridge.listURLs(category: selectedCategory)
                return
            }

            let matches = try RustVaultBridge.searchURLs(query: query, limit: 10_000)
            bookmarks = selectedCategory == nil
                ? matches
                : matches.filter { ($0.folderPath ?? "") == selectedCategory }
        } catch {
            fail(error)
        }
    }

    func add(
        url: String,
        title: String,
        note: String,
        category: String,
        tags: String,
        aliases: String,
        intents: String
    ) -> Bool {
        let request = SaveURLRequest(
            url: url,
            title: title.nilIfBlank,
            description: nil,
            note: note.nilIfBlank,
            tags: tags.csvValues,
            aliases: aliases.csvValues,
            intents: intents.csvValues,
            sourceType: "native_app",
            sourceBrowser: nil,
            sourceProfile: nil,
            folderPath: category.nilIfBlank,
            preferredBrowser: nil,
            project: nil,
            status: "active"
        )
        do {
            let result = try RustVaultBridge.saveURL(request)
            try reload()
            activityMessage = "Saved \(result.item.displayTitle)"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func update(
        bookmark: VaultBookmark,
        url: String,
        title: String,
        note: String,
        category: String,
        tags: String,
        aliases: String,
        intents: String
    ) -> Bool {
        do {
            let result = try RustVaultBridge.updateURL(
                target: bookmark.id,
                changes: UpdateURLRequest(
                    url: url,
                    title: title,
                    description: nil,
                    note: note,
                    tags: tags.csvValues,
                    aliases: aliases.csvValues,
                    intents: intents.csvValues,
                    folderPath: category,
                    preferredBrowser: nil,
                    project: nil
                )
            )
            try reload()
            activityMessage = "Updated \(result.item.displayTitle)"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func archive(_ bookmark: VaultBookmark) {
        do {
            _ = try RustVaultBridge.archiveURL(target: bookmark.id)
            try reload()
            activityMessage = "Archived \(bookmark.displayTitle)"
        } catch {
            fail(error)
        }
    }

    func delete(_ bookmark: VaultBookmark) -> Bool {
        do {
            _ = try RustVaultBridge.deleteURL(target: bookmark.id)
            try reload()
            activityMessage = "Deleted \(bookmark.displayTitle)"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func open(_ bookmark: VaultBookmark) {
        do {
            _ = try RustVaultBridge.openURL(target: bookmark.id)
            try reload()
        } catch {
            fail(error)
        }
    }

    func createCategory(path: String, note: String) -> Bool {
        let normalized = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        do {
            _ = try RustVaultBridge.createCategory(
                path: normalized,
                label: nil,
                note: note.nilIfBlank
            )
            try reload()
            selectedCategory = normalized
            try reload()
            activityMessage = "Created \(normalized)"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func renameCategory(_ category: VaultCategory, to newPath: String) -> Bool {
        let normalized = newPath.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        do {
            _ = try RustVaultBridge.renameCategory(oldPath: category.path, newPath: normalized)
            if selectedCategory == category.path {
                selectedCategory = normalized
            }
            try reload()
            activityMessage = "Renamed \(category.display)"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func archiveCategory(_ category: VaultCategory) -> Bool {
        do {
            _ = try RustVaultBridge.archiveCategory(path: category.path, moveTo: "")
            if selectedCategory == category.path {
                selectedCategory = nil
            }
            try reload()
            activityMessage = "Archived \(category.display); its URLs moved to Unfiled"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func prepareImport(from fileURL: URL, stripCommonRoot: Bool) -> PreparedVaultImport? {
        do {
            let accessing = fileURL.startAccessingSecurityScopedResource()
            defer {
                if accessing {
                    fileURL.stopAccessingSecurityScopedResource()
                }
            }
            let html = try String(contentsOf: fileURL, encoding: .utf8)
            let preview = try RustVaultBridge.previewImport(
                html: html,
                fileName: fileURL.lastPathComponent,
                stripCommonRoot: stripCommonRoot
            )
            return PreparedVaultImport(
                html: html,
                fileName: fileURL.lastPathComponent,
                preview: preview
            )
        } catch {
            fail(error)
            return nil
        }
    }

    func applyImport(
        _ prepared: PreparedVaultImport,
        sourceBrowser: String,
        sourceProfile: String,
        mode: String,
        stripCommonRoot: Bool
    ) -> Bool {
        do {
            let result = try RustVaultBridge.applyImport(
                ApplyImportRequest(
                    html: prepared.html,
                    fileName: prepared.fileName,
                    sourceBrowser: sourceBrowser,
                    sourceProfile: sourceProfile,
                    preferredBrowser: nil,
                    mode: mode,
                    stripCommonRoot: stripCommonRoot,
                    expectedSha256: prepared.preview.contentSha256,
                    confirmReset: mode == "reset"
                )
            )
            selectedCategory = nil
            searchText = ""
            try reload()
            activityMessage = "Imported \(result.created) new and \(result.updated) existing URLs"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func snapshot(for bookmark: VaultBookmark) -> VaultSnapshot? {
        do {
            return try RustVaultBridge.getSnapshot(target: bookmark.id).snapshot
        } catch {
            fail(error)
            return nil
        }
    }

    func clearMessages() {
        errorMessage = nil
        activityMessage = nil
    }

    private func fail(_ error: Error) {
        errorMessage = error.localizedDescription
    }

    private func reloadAfterExternalChange() {
        do {
            try reload()
        } catch {
            fail(error)
        }
    }
}

extension String {
    var nilIfBlank: String? {
        let value = trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    var csvValues: [String] {
        split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
    }
}
