import Testing
@testable import CodexURLVault
import Foundation

@Test
func englishAndJapaneseLabelsAreAvailable() {
    let english = VaultStrings(language: .english)
    let japanese = VaultStrings(language: .japanese)

    #expect(english.allURLs == "All URLs")
    #expect(japanese.allURLs == "すべてのURL")
}

@Test
func rustSnakeCasePayloadDecodesAcronymFields() throws {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    let data = Data(
        """
        {
          "id": "bookmark-1",
          "url": "https://example.test/",
          "canonical_url": "https://example.test/",
          "title": "Example",
          "description": null,
          "note": null,
          "tags": [],
          "aliases": [],
          "intents": [],
          "source_type": "browser_import",
          "source_browser": "brave",
          "source_profile": "Default",
          "folder_path": "Docs",
          "preferred_browser": "brave",
          "project": null,
          "status": "active",
          "created_at": "2026-07-30T00:00:00Z",
          "updated_at": "2026-07-30T00:00:00Z",
          "first_seen_at": null,
          "last_seen_at": null,
          "last_opened_at": null,
          "open_count": 0,
          "seen_in_latest_import": true,
          "missing_count": 0
        }
        """.utf8
    )

    let bookmark = try decoder.decode(VaultBookmark.self, from: data)
    #expect(bookmark.canonicalUrl == "https://example.test/")
}
