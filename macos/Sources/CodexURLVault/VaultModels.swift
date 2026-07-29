import Foundation

struct VaultBookmark: Codable, Identifiable, Hashable {
    let id: String
    let url: String
    let canonicalUrl: String
    var title: String?
    var description: String?
    var note: String?
    var tags: [String]
    var aliases: [String]
    var intents: [String]
    let sourceType: String
    let sourceBrowser: String?
    let sourceProfile: String?
    var folderPath: String?
    var preferredBrowser: String?
    var project: String?
    let status: String
    let createdAt: String
    let updatedAt: String
    let firstSeenAt: String?
    let lastSeenAt: String?
    let lastOpenedAt: String?
    let openCount: Int
    let seenInLatestImport: Bool
    let missingCount: Int
    let snapshot: VaultSnapshot?
    let score: Int?

    var displayTitle: String {
        let candidate = title?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return candidate.isEmpty ? url : candidate
    }
}

struct VaultCategory: Codable, Identifiable, Hashable {
    let path: String
    let label: String
    let note: String?
    let status: String
    let createdAt: String?
    let updatedAt: String?
    let display: String
    let count: Int

    var id: String { path }
}

struct VaultSnapshot: Codable, Hashable {
    let bookmarkId: String
    let artifactPath: String
    let sha256: String
    let kind: String
    let title: String?
    let capturedAt: String?
    let byteCount: Int?
    let status: String
    let verifiedAt: String?
    let updatedAt: String
    let preview: String?
    let content: String?
}

struct VaultMutation<Item: Codable>: Codable {
    let item: Item
    let canonicalHtml: String
}

struct VaultInitResult: Codable {
    let db: String
    let bookmarks: Int
}

struct VaultImportPreview: Codable {
    let fileName: String
    let contentSha256: String
    let parsedCount: Int
    let uniqueCount: Int
    let duplicateCount: Int
    let folderCount: Int
    let folders: [String]
    let commonRoot: String?
    let strippedRoot: String?
}

struct VaultImportResult: Codable {
    let created: Int
    let updated: Int
    let skipped: Int
    let canonicalHtml: String
}

struct VaultSnapshotIdentity: Codable {
    let id: String
    let title: String?
    let url: String
}

struct VaultSnapshotPayload: Codable {
    let bookmark: VaultSnapshotIdentity
    let snapshot: VaultSnapshot?
}

struct VaultOpenResult: Codable {
    let url: String
    let browser: String
    let bookmark: VaultBookmark?
    let dryRun: Bool
}

struct SaveURLRequest: Codable {
    let url: String
    let title: String?
    let description: String?
    let note: String?
    let tags: [String]
    let aliases: [String]
    let intents: [String]
    let sourceType: String?
    let sourceBrowser: String?
    let sourceProfile: String?
    let folderPath: String?
    let preferredBrowser: String?
    let project: String?
    let status: String?
}

struct UpdateURLRequest: Codable {
    let url: String?
    let title: String?
    let description: String?
    let note: String?
    let tags: [String]?
    let aliases: [String]?
    let intents: [String]?
    let folderPath: String?
    let preferredBrowser: String?
    let project: String?
}

struct ApplyImportRequest: Codable {
    let html: String
    let fileName: String
    let sourceBrowser: String
    let sourceProfile: String
    let preferredBrowser: String?
    let mode: String
    let stripCommonRoot: Bool
    let expectedSha256: String
    let confirmReset: Bool
}

struct SaveSnapshotRequest: Codable {
    let target: String
    let content: String
    let kind: String
    let title: String?
    let capturedAt: String?
}
