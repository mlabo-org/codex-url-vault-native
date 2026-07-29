import Combine
import Foundation

@MainActor
final class VaultStore: ObservableObject {
    @Published private(set) var bookmarks: [VaultBookmark] = []
    @Published private(set) var categories: [VaultCategory] = []
    @Published var selectedCategory: String?
    @Published var selectedBookmarkID: String?
    @Published var searchText = ""
    @Published var errorMessage: String?
    @Published var activityMessage: String?

    var selectedBookmark: VaultBookmark? {
        bookmarks.first { $0.id == selectedBookmarkID }
    }

    func start() {
        do {
            _ = try RustVaultBridge.initialize()
            try reload()
        } catch {
            fail(error)
        }
    }

    func reload() throws {
        categories = try RustVaultBridge.listCategories()
        bookmarks = try RustVaultBridge.listURLs(category: selectedCategory)
        if !bookmarks.contains(where: { $0.id == selectedBookmarkID }) {
            selectedBookmarkID = bookmarks.first?.id
        }
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
            bookmarks = query.isEmpty
                ? try RustVaultBridge.listURLs(category: selectedCategory)
                : try RustVaultBridge.searchURLs(query: query)
            selectedBookmarkID = bookmarks.first?.id
        } catch {
            fail(error)
        }
    }

    func add(
        url: String,
        title: String,
        note: String,
        category: String,
        tags: String
    ) -> Bool {
        let request = SaveURLRequest(
            url: url,
            title: title.nilIfBlank,
            description: nil,
            note: note.nilIfBlank,
            tags: tags.csvValues,
            aliases: [],
            intents: [],
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
            selectedBookmarkID = result.item.id
            activityMessage = "Saved \(result.item.displayTitle)"
            return true
        } catch {
            fail(error)
            return false
        }
    }

    func update(
        bookmark: VaultBookmark,
        title: String,
        note: String,
        category: String,
        tags: String
    ) -> Bool {
        do {
            let result = try RustVaultBridge.updateURL(
                target: bookmark.id,
                changes: UpdateURLRequest(
                    url: nil,
                    title: title,
                    description: nil,
                    note: note,
                    tags: tags.csvValues,
                    aliases: nil,
                    intents: nil,
                    folderPath: category,
                    preferredBrowser: nil,
                    project: nil
                )
            )
            try reload()
            selectedBookmarkID = result.item.id
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

    func open(_ bookmark: VaultBookmark) {
        do {
            _ = try RustVaultBridge.openURL(target: bookmark.id)
            try reload()
        } catch {
            fail(error)
        }
    }

    func importBookmarks(from fileURL: URL, stripCommonRoot: Bool = true) {
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
            let result = try RustVaultBridge.applyImport(
                ApplyImportRequest(
                    html: html,
                    fileName: fileURL.lastPathComponent,
                    sourceBrowser: "browser",
                    sourceProfile: "",
                    preferredBrowser: nil,
                    mode: "merge",
                    stripCommonRoot: stripCommonRoot,
                    expectedSha256: preview.contentSha256,
                    confirmReset: false
                )
            )
            try reload()
            activityMessage = "Imported \(result.created) new and \(result.updated) existing URLs"
        } catch {
            fail(error)
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
}

private extension String {
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
