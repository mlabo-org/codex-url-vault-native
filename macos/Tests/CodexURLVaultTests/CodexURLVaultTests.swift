import Testing
@testable import CodexURLVault

@Test
func englishAndJapaneseLabelsAreAvailable() {
    let english = VaultStrings(language: .english)
    let japanese = VaultStrings(language: .japanese)

    #expect(english.allURLs == "All URLs")
    #expect(japanese.allURLs == "すべてのURL")
}
