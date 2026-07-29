import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct VaultView: View {
    @Environment(\.appShellLanguage) private var language
    @Environment(\.colorScheme) private var colorScheme
    @AppStorage(AppShellPreferenceKeys.language)
    private var selectedLanguage = AppShellLanguage.system
    @AppStorage(AppShellPreferenceKeys.theme)
    private var selectedTheme = AppShellTheme.system
    @StateObject private var store = VaultStore()
    @State private var editingBookmark: VaultBookmark?
    @State private var snapshotBookmark: VaultBookmark?
    @State private var editingCategory: VaultCategory?
    @State private var showingAdd = false
    @State private var showingAddCategory = false
    @State private var showingImport = false
    @State private var showingPrompts = false

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    private var palette: VaultPalette {
        VaultPalette(colorScheme: colorScheme)
    }

    var body: some View {
        HSplitView {
            VaultSidebar(
                store: store,
                strings: strings,
                palette: palette,
                editCategory: { editingCategory = $0 }
            )
            .frame(minWidth: 220, idealWidth: 280, maxWidth: 430)

            VaultMainContent(
                store: store,
                strings: strings,
                palette: palette,
                selectedLanguage: $selectedLanguage,
                selectedTheme: $selectedTheme,
                showingAdd: $showingAdd,
                showingAddCategory: $showingAddCategory,
                showingImport: $showingImport,
                showingPrompts: $showingPrompts,
                editingBookmark: $editingBookmark,
                snapshotBookmark: $snapshotBookmark
            )
            .frame(minWidth: 480, maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(palette.background)
        .frame(minWidth: 760, minHeight: 580)
        .sheet(isPresented: $showingAdd) {
            BookmarkEditorView(store: store, bookmark: nil)
        }
        .sheet(item: $editingBookmark) { bookmark in
            BookmarkEditorView(store: store, bookmark: bookmark)
        }
        .sheet(item: $snapshotBookmark) { bookmark in
            SnapshotViewer(store: store, bookmark: bookmark)
        }
        .sheet(isPresented: $showingAddCategory) {
            CategoryEditorView(store: store, category: nil)
        }
        .sheet(item: $editingCategory) { category in
            CategoryEditorView(store: store, category: category)
        }
        .sheet(isPresented: $showingImport) {
            ImportWizardView(store: store)
        }
        .sheet(isPresented: $showingPrompts) {
            PromptSamplesView()
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
                    .padding(.horizontal, 16)
                    .padding(.vertical, 9)
                    .background(.regularMaterial, in: Capsule())
                    .padding()
                    .onTapGesture { store.activityMessage = nil }
            }
        }
        .task { store.start() }
    }
}

private struct VaultSidebar: View {
    @ObservedObject var store: VaultStore
    let strings: VaultStrings
    let palette: VaultPalette
    let editCategory: (VaultCategory) -> Void

    private var unfiledCount: Int {
        max(0, store.totalCount - store.categories.reduce(0) { $0 + $1.count })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Text("URL")
                    .appFont(.caption, weight: .bold)
                    .foregroundStyle(palette.accent)
                    .frame(width: 42, height: 42)
                    .background(palette.accent.opacity(0.12), in: RoundedRectangle(cornerRadius: 9))
                    .overlay {
                        RoundedRectangle(cornerRadius: 9)
                            .stroke(palette.accent.opacity(0.45))
                    }
                VStack(alignment: .leading, spacing: 2) {
                    Text("Bookmark Vault")
                        .appFont(.headline)
                    Text(strings.localVaultSubtitle)
                        .appFont(.caption)
                        .foregroundStyle(palette.secondaryText)
                }
            }
            .padding(.horizontal, 18)
            .padding(.top, 20)
            .padding(.bottom, 22)

            Text(strings.search)
                .appFont(.caption, weight: .semibold)
                .foregroundStyle(palette.secondaryText)
                .padding(.horizontal, 18)
                .padding(.bottom, 7)

            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(palette.secondaryText)
                TextField(strings.searchExample, text: $store.searchText)
                    .textFieldStyle(.plain)
                    .onSubmit { store.search() }
                    .onChange(of: store.searchText) { _, _ in
                        store.search()
                    }
                if !store.searchText.isEmpty {
                    Button {
                        store.searchText = ""
                        store.search()
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(palette.secondaryText)
                }
            }
            .padding(.horizontal, 11)
            .frame(height: 40)
            .background(palette.panel, in: RoundedRectangle(cornerRadius: 8))
            .overlay {
                RoundedRectangle(cornerRadius: 8).stroke(palette.border)
            }
            .padding(.horizontal, 18)

            Text(strings.folders)
                .appFont(.caption, weight: .semibold)
                .foregroundStyle(palette.secondaryText)
                .padding(.horizontal, 18)
                .padding(.top, 24)
                .padding(.bottom, 8)

            ScrollView {
                LazyVStack(spacing: 4) {
                    SidebarRow(
                        title: strings.all,
                        count: store.totalCount,
                        selected: store.selectedCategory == nil,
                        palette: palette
                    ) {
                        store.selectCategory(nil)
                    }

                    SidebarRow(
                        title: strings.uncategorized,
                        count: unfiledCount,
                        selected: store.selectedCategory == "",
                        palette: palette
                    ) {
                        store.selectCategory("")
                    }

                    ForEach(store.categories) { category in
                        HStack(spacing: 4) {
                            SidebarRow(
                                title: category.display,
                                count: category.count,
                                selected: store.selectedCategory == category.path,
                                palette: palette
                            ) {
                                store.selectCategory(category.path)
                            }

                            Button {
                                editCategory(category)
                            } label: {
                                Image(systemName: "ellipsis")
                                    .frame(width: 28, height: 28)
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(palette.secondaryText)
                            .accessibilityLabel(strings.editCategory)
                        }
                    }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 16)
            }
        }
        .foregroundStyle(palette.primaryText)
        .background(palette.sidebar)
    }
}

private struct SidebarRow: View {
    let title: String
    let count: Int
    let selected: Bool
    let palette: VaultPalette
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Text(title)
                    .lineLimit(1)
                Spacer(minLength: 8)
                Text("\(count)")
                    .appFont(.caption)
                    .foregroundStyle(palette.secondaryText)
            }
            .padding(.horizontal, 11)
            .frame(height: 38)
            .background(
                selected ? palette.selection : Color.clear,
                in: RoundedRectangle(cornerRadius: 8)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

private struct VaultMainContent: View {
    @ObservedObject var store: VaultStore
    let strings: VaultStrings
    let palette: VaultPalette
    @Binding var selectedLanguage: AppShellLanguage
    @Binding var selectedTheme: AppShellTheme
    @Binding var showingAdd: Bool
    @Binding var showingAddCategory: Bool
    @Binding var showingImport: Bool
    @Binding var showingPrompts: Bool
    @Binding var editingBookmark: VaultBookmark?
    @Binding var snapshotBookmark: VaultBookmark?

    private var title: String {
        guard let selected = store.selectedCategory else {
            return strings.allBookmarks
        }
        return selected.isEmpty ? strings.uncategorized : selected
    }

    private var gridColumns: [GridItem] {
        [GridItem(.adaptive(minimum: 250, maximum: 390), spacing: 14, alignment: .top)]
    }

    var body: some View {
        VStack(spacing: 0) {
            responsiveHeader
                .padding(.horizontal, 28)
                .padding(.vertical, 20)

            Divider()
                .overlay(palette.border)
                .padding(.horizontal, 28)

            if store.bookmarks.isEmpty {
                ContentUnavailableView(
                    store.searchText.isEmpty ? strings.noBookmarks : strings.noSearchResults,
                    systemImage: store.searchText.isEmpty ? "bookmark" : "magnifyingglass",
                    description: Text(
                        store.searchText.isEmpty
                            ? strings.noBookmarksHint
                            : strings.noSearchResultsHint
                    )
                )
                .foregroundStyle(palette.primaryText)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVGrid(columns: gridColumns, alignment: .leading, spacing: 14) {
                        ForEach(store.bookmarks) { bookmark in
                            BookmarkCard(
                                bookmark: bookmark,
                                strings: strings,
                                palette: palette,
                                open: { store.open(bookmark) },
                                showSnapshot: { snapshotBookmark = bookmark },
                                edit: { editingBookmark = bookmark }
                            )
                        }
                    }
                    .padding(28)
                }
            }
        }
        .foregroundStyle(palette.primaryText)
        .background(palette.background)
    }

    private var responsiveHeader: some View {
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 24) {
                titleBlock
                Spacer(minLength: 20)
                toolbarControls
            }
            VStack(alignment: .leading, spacing: 16) {
                titleBlock
                toolbarControls
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private var titleBlock: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(strings.localURLIndex)
                .appFont(.caption, weight: .bold)
                .foregroundStyle(palette.accent)
            Text(title)
                .appFont(.title, weight: .bold)
                .lineLimit(2)
        }
    }

    private var toolbarControls: some View {
        VStack(alignment: .trailing, spacing: 10) {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    languageControl
                    themeControl
                }
                VStack(alignment: .leading, spacing: 8) {
                    languageControl
                    themeControl
                }
            }

            ViewThatFits(in: .horizontal) {
                HStack(spacing: 8) {
                    actionButtons
                }
                VStack(alignment: .trailing, spacing: 8) {
                    actionButtons
                }
            }
        }
    }

    @ViewBuilder
    private var actionButtons: some View {
        VaultToolbarButton(strings.promptSamples, systemImage: "text.bubble") {
            showingPrompts = true
        }
        VaultToolbarButton(strings.importBookmarks, systemImage: "square.and.arrow.down") {
            showingImport = true
        }
        VaultToolbarButton(strings.addURL, systemImage: "plus") {
            showingAdd = true
        }
        VaultToolbarButton(strings.addCategory, systemImage: "folder.badge.plus") {
            showingAddCategory = true
        }
        Text("\(store.bookmarks.count) / \(store.totalCount)")
            .appFont(.headline)
            .monospacedDigit()
            .padding(.horizontal, 12)
            .frame(height: 32)
            .background(palette.panel, in: RoundedRectangle(cornerRadius: 7))
            .overlay {
                RoundedRectangle(cornerRadius: 7).stroke(palette.border)
            }
    }

    private var languageControl: some View {
        Picker(strings.language, selection: $selectedLanguage) {
            Text("System").tag(AppShellLanguage.system)
            Text("Japanese").tag(AppShellLanguage.japanese)
            Text("English").tag(AppShellLanguage.english)
        }
        .pickerStyle(.segmented)
        .fixedSize()
        .accessibilityLabel(strings.language)
    }

    private var themeControl: some View {
        Picker(strings.theme, selection: $selectedTheme) {
            Text("System").tag(AppShellTheme.system)
            Text("Light").tag(AppShellTheme.light)
            Text("Dark").tag(AppShellTheme.dark)
        }
        .pickerStyle(.segmented)
        .fixedSize()
        .accessibilityLabel(strings.theme)
    }
}

private struct VaultToolbarButton: View {
    let title: String
    let systemImage: String
    let action: () -> Void

    init(_ title: String, systemImage: String, action: @escaping () -> Void) {
        self.title = title
        self.systemImage = systemImage
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .lineLimit(1)
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
    }
}

private struct BookmarkCard: View {
    let bookmark: VaultBookmark
    let strings: VaultStrings
    let palette: VaultPalette
    let open: () -> Void
    let showSnapshot: () -> Void
    let edit: () -> Void

    private var category: String {
        let path = bookmark.folderPath ?? ""
        return path.isEmpty ? strings.uncategorized : path
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button(action: open) {
                VStack(alignment: .leading, spacing: 7) {
                    Text(bookmark.displayTitle)
                        .appFont(.headline)
                        .foregroundStyle(palette.primaryText)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                    Text(bookmark.url)
                        .appFont(.caption)
                        .foregroundStyle(palette.accent)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)

            HStack(spacing: 7) {
                VaultBadge(text: category, palette: palette)
                if bookmark.snapshot?.status == "valid" {
                    VaultBadge(
                        text: strings.savedSnapshot,
                        palette: palette,
                        emphasized: true
                    )
                }
            }

            if !bookmark.aliases.isEmpty {
                Text(bookmark.aliases.prefix(3).joined(separator: " · "))
                    .appFont(.caption)
                    .foregroundStyle(palette.secondaryText)
                    .lineLimit(1)
            }

            if !bookmark.tags.isEmpty {
                Text(bookmark.tags.prefix(4).map { "#\($0)" }.joined(separator: "  "))
                    .appFont(.caption)
                    .foregroundStyle(palette.secondaryText)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)

            ViewThatFits(in: .horizontal) {
                HStack(spacing: 7) {
                    cardActions
                }
                VStack(alignment: .leading, spacing: 7) {
                    cardActions
                }
            }
        }
        .padding(15)
        .frame(maxWidth: .infinity, minHeight: 190, alignment: .topLeading)
        .background(palette.panel, in: RoundedRectangle(cornerRadius: 9))
        .overlay {
            RoundedRectangle(cornerRadius: 9)
                .stroke(palette.border)
        }
        .contextMenu {
            Button(strings.open, action: open)
            Button(strings.edit, action: edit)
        }
    }

    @ViewBuilder
    private var cardActions: some View {
        Button(strings.open, action: open)
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
        if bookmark.snapshot != nil {
            Button(strings.savedSnapshot, action: showSnapshot)
                .buttonStyle(.bordered)
                .controlSize(.small)
        }
        Button(strings.edit, action: edit)
            .buttonStyle(.bordered)
            .controlSize(.small)
    }
}

private struct VaultBadge: View {
    let text: String
    let palette: VaultPalette
    var emphasized = false

    var body: some View {
        Text(text)
            .appFont(.caption, weight: .medium)
            .foregroundStyle(emphasized ? palette.accent : palette.secondaryText)
            .lineLimit(1)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(
                emphasized ? palette.accent.opacity(0.12) : palette.background.opacity(0.75),
                in: Capsule()
            )
    }
}

private struct BookmarkEditorView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.appShellLanguage) private var language
    @ObservedObject var store: VaultStore
    let bookmark: VaultBookmark?
    @State private var url: String
    @State private var title: String
    @State private var category: String
    @State private var aliases: String
    @State private var tags: String
    @State private var intents: String
    @State private var note: String
    @State private var confirmArchive = false

    init(store: VaultStore, bookmark: VaultBookmark?) {
        self.store = store
        self.bookmark = bookmark
        _url = State(initialValue: bookmark?.url ?? "")
        _title = State(initialValue: bookmark?.title ?? "")
        _category = State(initialValue: bookmark?.folderPath ?? "")
        _aliases = State(initialValue: bookmark?.aliases.joined(separator: ", ") ?? "")
        _tags = State(initialValue: bookmark?.tags.joined(separator: ", ") ?? "")
        _intents = State(initialValue: bookmark?.intents.joined(separator: ", ") ?? "")
        _note = State(initialValue: bookmark?.note ?? "")
    }

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                Text(bookmark == nil ? strings.addURL : strings.editURL)
                    .appFont(.title2, weight: .bold)
                Spacer()
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.bordered)
            }

            ScrollView {
                VStack(spacing: 14) {
                    field("URL", text: $url)
                    field(strings.title, text: $title)
                    field(strings.category, text: $category, prompt: "Research/AI")
                    field(strings.aliases, text: $aliases, prompt: strings.csvHint)
                    field(strings.tags, text: $tags, prompt: strings.csvHint)
                    field(strings.intents, text: $intents, prompt: strings.csvHint)

                    VStack(alignment: .leading, spacing: 6) {
                        Text(strings.note)
                            .appFont(.caption, weight: .semibold)
                            .foregroundStyle(.secondary)
                        TextEditor(text: $note)
                            .frame(minHeight: 110)
                            .padding(7)
                            .background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 7))
                    }
                }
            }

            HStack {
                if bookmark != nil {
                    Button(strings.archive, role: .destructive) {
                        confirmArchive = true
                    }
                }
                Spacer()
                Button(strings.cancel) { dismiss() }
                Button(strings.save) {
                    save()
                }
                .buttonStyle(.borderedProminent)
                .disabled(url.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(24)
        .frame(width: 620, height: 650)
        .confirmationDialog(
            strings.archiveURLTitle,
            isPresented: $confirmArchive,
            titleVisibility: .visible
        ) {
            Button(strings.archive, role: .destructive) {
                if let bookmark {
                    store.archive(bookmark)
                }
                dismiss()
            }
            Button(strings.cancel, role: .cancel) {}
        }
    }

    private func field(
        _ label: String,
        text: Binding<String>,
        prompt: String = ""
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .appFont(.caption, weight: .semibold)
                .foregroundStyle(.secondary)
            TextField(prompt, text: text)
                .textFieldStyle(.roundedBorder)
        }
    }

    private func save() {
        let succeeded: Bool
        if let bookmark {
            succeeded = store.update(
                bookmark: bookmark,
                url: url,
                title: title,
                note: note,
                category: category,
                tags: tags,
                aliases: aliases,
                intents: intents
            )
        } else {
            succeeded = store.add(
                url: url,
                title: title,
                note: note,
                category: category,
                tags: tags,
                aliases: aliases,
                intents: intents
            )
        }
        if succeeded {
            dismiss()
        }
    }
}

private struct CategoryEditorView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.appShellLanguage) private var language
    @ObservedObject var store: VaultStore
    let category: VaultCategory?
    @State private var path: String
    @State private var note: String
    @State private var confirmArchive = false

    init(store: VaultStore, category: VaultCategory?) {
        self.store = store
        self.category = category
        _path = State(initialValue: category?.path ?? "")
        _note = State(initialValue: category?.note ?? "")
    }

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text(category == nil ? strings.addCategory : strings.editCategory)
                .appFont(.title2, weight: .bold)

            TextField("Research/AI", text: $path)
                .textFieldStyle(.roundedBorder)
            Text(strings.categoryPathHint)
                .appFont(.caption)
                .foregroundStyle(.secondary)
            TextField(strings.note, text: $note, axis: .vertical)
                .lineLimit(3...6)

            HStack {
                if category != nil {
                    Button(strings.deleteCategory, role: .destructive) {
                        confirmArchive = true
                    }
                }
                Spacer()
                Button(strings.cancel) { dismiss() }
                Button(strings.save) {
                    let succeeded: Bool
                    if let category {
                        succeeded = store.renameCategory(category, to: path)
                    } else {
                        succeeded = store.createCategory(path: path, note: note)
                    }
                    if succeeded {
                        dismiss()
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(24)
        .frame(width: 500)
        .confirmationDialog(
            strings.deleteCategoryTitle,
            isPresented: $confirmArchive,
            titleVisibility: .visible
        ) {
            Button(strings.deleteCategory, role: .destructive) {
                if let category, store.archiveCategory(category) {
                    dismiss()
                }
            }
            Button(strings.cancel, role: .cancel) {}
        } message: {
            Text(strings.deleteCategoryHint)
        }
    }
}

private struct ImportWizardView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.appShellLanguage) private var language
    @ObservedObject var store: VaultStore
    @State private var step = 1
    @State private var sourceBrowser = "brave"
    @State private var sourceProfile = ""
    @State private var stripCommonRoot = true
    @State private var importMode = "merge"
    @State private var confirmReset = false
    @State private var showingFilePicker = false
    @State private var prepared: PreparedVaultImport?

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text(strings.browserMigration)
                        .appFont(.caption, weight: .bold)
                        .foregroundStyle(.blue)
                    Text(strings.importBookmarks)
                        .appFont(.title2, weight: .bold)
                }
                Spacer()
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.bordered)
            }

            ImportStepper(step: step, strings: strings)

            Group {
                switch step {
                case 1:
                    browserStep
                case 2:
                    fileStep
                default:
                    confirmationStep
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

            Divider()

            HStack {
                if step > 1 {
                    Button(strings.back) { step -= 1 }
                }
                Spacer()
                Button(strings.cancel) { dismiss() }
                if step < 3 {
                    Button(strings.next) { step += 1 }
                        .buttonStyle(.borderedProminent)
                        .disabled(step == 2 && prepared == nil)
                } else {
                    Button(strings.runImport) {
                        guard let prepared else { return }
                        if store.applyImport(
                            prepared,
                            sourceBrowser: sourceBrowser,
                            sourceProfile: sourceProfile,
                            mode: importMode,
                            stripCommonRoot: stripCommonRoot
                        ) {
                            dismiss()
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(prepared == nil || (importMode == "reset" && !confirmReset))
                }
            }
        }
        .padding(24)
        .frame(width: 700, height: 620)
        .fileImporter(
            isPresented: $showingFilePicker,
            allowedContentTypes: [.html],
            allowsMultipleSelection: false
        ) { result in
            switch result {
            case .success(let files):
                if let file = files.first {
                    prepared = store.prepareImport(
                        from: file,
                        stripCommonRoot: stripCommonRoot
                    )
                }
            case .failure(let error):
                store.errorMessage = error.localizedDescription
            }
        }
    }

    private var browserStep: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(strings.whichBrowser)
                .appFont(.title3, weight: .semibold)
            Text(strings.browserChoiceHint)
                .foregroundStyle(.secondary)

            Picker(strings.browser, selection: $sourceBrowser) {
                Text("Brave").tag("brave")
                Text("Chrome").tag("chrome")
                Text("Firefox").tag("firefox")
                Text("Safari").tag("safari")
                Text(strings.otherBrowser).tag("other")
            }
            .pickerStyle(.segmented)

            GroupBox(strings.exportHTML) {
                Text(exportGuide)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(6)
            }

            TextField(strings.profileOptional, text: $sourceProfile)
                .textFieldStyle(.roundedBorder)
        }
    }

    private var fileStep: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(strings.selectExportedHTML)
                .appFont(.title3, weight: .semibold)
            Text(strings.filePrivacy)
                .foregroundStyle(.secondary)

            Toggle(strings.stripCommonRoot, isOn: $stripCommonRoot)
                .onChange(of: stripCommonRoot) { _, _ in
                    prepared = nil
                }

            Button {
                showingFilePicker = true
            } label: {
                Label(strings.chooseHTML, systemImage: "doc.badge.plus")
            }
            .buttonStyle(.borderedProminent)

            if let preview = prepared?.preview {
                GroupBox(strings.preview) {
                    Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 10) {
                        previewRow(strings.fileName, preview.fileName)
                        previewRow(strings.bookmarks, "\(preview.uniqueCount)")
                        previewRow(strings.duplicates, "\(preview.duplicateCount)")
                        previewRow(strings.folders, "\(preview.folderCount)")
                        if let root = preview.strippedRoot ?? preview.commonRoot {
                            previewRow(strings.commonRoot, root)
                        }
                    }
                    .padding(6)
                }
            }
        }
    }

    private var confirmationStep: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(strings.confirmImport)
                .appFont(.title3, weight: .semibold)

            Picker(strings.importMode, selection: $importMode) {
                Text(strings.mergeRecommended).tag("merge")
                Text(strings.addOnly).tag("add")
                Text(strings.resetAdvanced).tag("reset")
            }
            .pickerStyle(.radioGroup)

            Text(modeExplanation)
                .foregroundStyle(.secondary)

            if importMode == "reset" {
                Toggle(strings.confirmReset, isOn: $confirmReset)
                    .foregroundStyle(.red)
            }

            if let preview = prepared?.preview {
                GroupBox(strings.importSummary) {
                    VStack(alignment: .leading, spacing: 7) {
                        Text("\(preview.uniqueCount) \(strings.bookmarks)")
                        Text("\(preview.folderCount) \(strings.folders)")
                        Text("\(sourceBrowser.capitalized) · \(preview.fileName)")
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(6)
                }
            }
        }
    }

    private func previewRow(_ label: String, _ value: String) -> some View {
        GridRow {
            Text(label).foregroundStyle(.secondary)
            Text(value).textSelection(.enabled)
        }
    }

    private var exportGuide: String {
        switch sourceBrowser {
        case "brave", "chrome":
            return strings.chromiumExportGuide
        case "firefox":
            return strings.firefoxExportGuide
        case "safari":
            return strings.safariExportGuide
        default:
            return strings.otherExportGuide
        }
    }

    private var modeExplanation: String {
        switch importMode {
        case "add":
            return strings.addOnlyHint
        case "reset":
            return strings.resetHint
        default:
            return strings.mergeHint
        }
    }
}

private struct ImportStepper: View {
    let step: Int
    let strings: VaultStrings

    var body: some View {
        HStack(spacing: 12) {
            stepItem(1, strings.browser)
            Rectangle().frame(height: 1).foregroundStyle(.quaternary)
            stepItem(2, strings.htmlFile)
            Rectangle().frame(height: 1).foregroundStyle(.quaternary)
            stepItem(3, strings.confirm)
        }
    }

    private func stepItem(_ number: Int, _ label: String) -> some View {
        HStack(spacing: 7) {
            Text("\(number)")
                .appFont(.caption, weight: .bold)
                .frame(width: 24, height: 24)
                .background(
                    number <= step ? Color.accentColor : Color.secondary.opacity(0.2),
                    in: Circle()
                )
            Text(label)
                .appFont(.caption, weight: .semibold)
        }
    }
}

private struct SnapshotViewer: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.appShellLanguage) private var language
    @ObservedObject var store: VaultStore
    let bookmark: VaultBookmark
    @State private var snapshot: VaultSnapshot?

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text(strings.savedSnapshot)
                        .appFont(.caption, weight: .bold)
                        .foregroundStyle(.blue)
                    Text(bookmark.displayTitle)
                        .appFont(.title2, weight: .bold)
                        .lineLimit(2)
                }
                Spacer()
                Button(strings.close) { dismiss() }
            }

            if let snapshot {
                HStack {
                    Text(snapshot.kind)
                    Text(snapshot.status)
                    Spacer()
                    if let count = snapshot.byteCount {
                        Text("\(count) bytes")
                    }
                }
                .appFont(.caption)
                .foregroundStyle(.secondary)

                ScrollView {
                    Text(snapshot.content ?? snapshot.preview ?? strings.snapshotUnavailable)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                        .padding()
                }
                .background(.quaternary.opacity(0.25), in: RoundedRectangle(cornerRadius: 8))
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(24)
        .frame(width: 760, height: 620)
        .task {
            snapshot = store.snapshot(for: bookmark)
        }
    }
}

private struct PromptSamplesView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.appShellLanguage) private var language

    private var strings: VaultStrings {
        VaultStrings(language: language)
    }

    private var samples: [String] {
        if language.resolved == .japanese {
            return [
                "$codex-url-vault を開いて",
                "$codex-url-vault に https://example.com を保存。タイトルは「Example」、エイリアスは example、タグは reference",
                "$codex-url-vault で保存済みURLにローカルのMarkdownスナップショットを追加して",
                "$codex-url-vault で「Cloudflare SSL」を検索して候補を3件出して",
                "$codex-url-vault に ~/Downloads/bookmarks.html を Brave のブックマークとして取り込んで",
                "$codex-url-vault で保存URLを一覧して、エイリアス、タグ、カテゴリを整理して",
            ]
        }
        return [
            "Use $codex-url-vault to open the local Vault.",
            "Use $codex-url-vault to save https://example.com with title Example, alias example, and tag reference.",
            "Use $codex-url-vault to attach a local Markdown snapshot to a saved URL.",
            "Use $codex-url-vault to search saved URLs and snapshots for Cloudflare SSL.",
            "Use $codex-url-vault to import ~/Downloads/bookmarks.html from Brave.",
            "Use $codex-url-vault to list saved URLs and organize aliases, tags, and categories.",
        ]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text(strings.promptSamples)
                    .appFont(.title2, weight: .bold)
                Spacer()
                Button(strings.close) { dismiss() }
            }

            ScrollView {
                LazyVStack(spacing: 10) {
                    ForEach(samples, id: \.self) { sample in
                        HStack(alignment: .top, spacing: 12) {
                            Text(sample)
                                .font(.system(.body, design: .monospaced))
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                            Button(strings.copy) {
                                NSPasteboard.general.clearContents()
                                NSPasteboard.general.setString(sample, forType: .string)
                            }
                            .buttonStyle(.bordered)
                        }
                        .padding(12)
                        .background(.quaternary.opacity(0.25), in: RoundedRectangle(cornerRadius: 8))
                    }
                }
            }
        }
        .padding(24)
        .frame(width: 760, height: 560)
    }
}

struct VaultPalette {
    let background: Color
    let sidebar: Color
    let panel: Color
    let selection: Color
    let border: Color
    let accent: Color
    let primaryText: Color
    let secondaryText: Color

    init(colorScheme: ColorScheme) {
        if colorScheme == .dark {
            background = Color(red: 0.043, green: 0.063, blue: 0.094)
            sidebar = Color(red: 0.059, green: 0.090, blue: 0.133)
            panel = Color(red: 0.078, green: 0.110, blue: 0.161)
            selection = Color(red: 0.105, green: 0.153, blue: 0.224)
            border = Color(red: 0.161, green: 0.204, blue: 0.278)
            accent = Color(red: 0.384, green: 0.659, blue: 1.0)
            primaryText = Color(red: 0.93, green: 0.95, blue: 0.98)
            secondaryText = Color(red: 0.60, green: 0.65, blue: 0.72)
        } else {
            background = Color(red: 0.957, green: 0.969, blue: 0.988)
            sidebar = Color.white
            panel = Color.white
            selection = Color(red: 0.886, green: 0.925, blue: 0.984)
            border = Color(red: 0.82, green: 0.85, blue: 0.90)
            accent = Color(red: 0.12, green: 0.40, blue: 0.82)
            primaryText = Color(red: 0.08, green: 0.11, blue: 0.16)
            secondaryText = Color(red: 0.38, green: 0.43, blue: 0.50)
        }
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
    var localVaultSubtitle: String { text("Codex用のローカルURL保管庫", "Local URL vault for Codex") }
    var localURLIndex: String { text("ローカルURL索引", "LOCAL URL INDEX") }
    var allURLs: String { text("すべてのURL", "All URLs") }
    var allBookmarks: String { text("すべてのブックマーク", "All Bookmarks") }
    var all: String { text("すべて", "All") }
    var categories: String { text("カテゴリー", "Categories") }
    var category: String { text("カテゴリ", "Category") }
    var folders: String { text("フォルダ", "Folders") }
    var uncategorized: String { text("未分類", "Unfiled") }
    var search: String { text("検索", "Search") }
    var searchPrompt: String { text("URLを検索", "Search URLs") }
    var searchExample: String { text("mlabo, SWELL, Cloudflare...", "mlabo, SWELL, Cloudflare...") }
    var addURL: String { text("URLを追加", "Add URL") }
    var editURL: String { text("URLを編集", "Edit URL") }
    var importBookmarks: String { text("ブックマークをインポート", "Import Bookmarks") }
    var noBookmarks: String { text("ブックマークがありません", "No Bookmarks") }
    var noBookmarksHint: String { text("URLを追加するか、ブラウザから読み込んでください。", "Add a URL or import browser bookmarks.") }
    var noSearchResults: String { text("一致するURLがありません", "No Matching URLs") }
    var noSearchResultsHint: String { text("検索語を変えてください。", "Try another search.") }
    var open: String { text("開く", "Open") }
    var archive: String { text("アーカイブ", "Archive") }
    var archiveURLTitle: String { text("このURLをアーカイブしますか？", "Archive this URL?") }
    var title: String { text("タイトル", "Title") }
    var tags: String { text("タグ", "Tags") }
    var aliases: String { text("エイリアス", "Aliases") }
    var intents: String { text("用途", "Intents") }
    var csvHint: String { text("カンマ区切り", "Comma separated") }
    var note: String { text("メモ", "Note") }
    var project: String { text("プロジェクト", "Project") }
    var opened: String { text("開いた回数", "Open count") }
    var savedSnapshot: String { text("保存版", "Saved copy") }
    var snapshotUnavailable: String { text("保存テキストを読み込めません。", "Saved content is unavailable.") }
    var edit: String { text("編集", "Edit") }
    var save: String { text("保存", "Save") }
    var cancel: String { text("キャンセル", "Cancel") }
    var close: String { text("閉じる", "Close") }
    var error: String { text("エラー", "Error") }
    var ok: String { "OK" }
    var language: String { text("言語", "Language") }
    var theme: String { text("テーマ", "Theme") }
    var promptSamples: String { text("プロンプト例", "Prompt Examples") }
    var copy: String { text("コピー", "Copy") }
    var addCategory: String { text("カテゴリを追加", "Add Category") }
    var editCategory: String { text("カテゴリを編集", "Edit Category") }
    var deleteCategory: String { text("カテゴリを削除", "Delete Category") }
    var deleteCategoryTitle: String { text("このカテゴリを削除しますか？", "Delete this category?") }
    var deleteCategoryHint: String { text("中のURLは削除されず、「未分類」へ移動します。", "Its URLs will move to Unfiled.") }
    var categoryPathHint: String { text("スラッシュで階層を作れます。", "Use slashes to create nested folders.") }
    var browserMigration: String { text("ブラウザ移行", "BROWSER MIGRATION") }
    var browser: String { text("ブラウザ", "Browser") }
    var htmlFile: String { text("HTMLファイル", "HTML File") }
    var confirm: String { text("確認", "Confirm") }
    var whichBrowser: String { text("どのブラウザから移しますか？", "Which browser are you moving from?") }
    var browserChoiceHint: String { text("選ぶと、HTMLを書き出す手順を表示します。", "Choose one to see its HTML export steps.") }
    var otherBrowser: String { text("その他", "Other") }
    var exportHTML: String { text("HTMLを書き出す", "Export HTML") }
    var chromiumExportGuide: String { text("ブックマークマネージャーを開き、メニューから「ブックマークをエクスポート」を選びます。", "Open Bookmark Manager and choose Export bookmarks from its menu.") }
    var firefoxExportGuide: String { text("ブックマーク管理画面の「インポートとバックアップ」からHTMLへ書き出します。", "In Manage Bookmarks, choose Import and Backup, then Export Bookmarks to HTML.") }
    var safariExportGuide: String { text("ファイルメニューから「書き出す」→「ブックマーク」を選びます。", "Choose File, Export, then Bookmarks.") }
    var otherExportGuide: String { text("ブラウザのブックマーク管理画面からHTML形式で書き出します。", "Export bookmarks as HTML from your browser's bookmark manager.") }
    var profileOptional: String { text("プロファイル名（任意）", "Profile name (optional)") }
    var selectExportedHTML: String { text("書き出したHTMLを選択", "Select the exported HTML") }
    var filePrivacy: String { text("ファイルはこのMac内のローカルVaultだけで処理されます。", "The file is processed only by the local Vault on this Mac.") }
    var stripCommonRoot: String { text("共通の最上位フォルダを取り除く", "Remove the common top-level folder") }
    var chooseHTML: String { text("HTMLファイルを選ぶ", "Choose HTML File") }
    var preview: String { text("プレビュー", "Preview") }
    var fileName: String { text("ファイル", "File") }
    var bookmarks: String { text("ブックマーク", "Bookmarks") }
    var duplicates: String { text("重複", "Duplicates") }
    var commonRoot: String { text("共通ルート", "Common root") }
    var confirmImport: String { text("取り込み内容を確認", "Confirm the import") }
    var importMode: String { text("取り込み方式", "Import mode") }
    var mergeRecommended: String { text("統合（推奨）", "Merge (recommended)") }
    var addOnly: String { text("追加", "Add") }
    var resetAdvanced: String { text("リセット（上級者向け）", "Reset (advanced)") }
    var mergeHint: String { text("既存URLを更新し、新しいURLを追加します。", "Update existing URLs and add new ones.") }
    var addOnlyHint: String { text("既存項目を保ったまま別項目として追加します。", "Keep existing items and add imported entries separately.") }
    var resetHint: String { text("現在のVaultを置き換えてから取り込みます。", "Replace the current Vault before importing.") }
    var confirmReset: String { text("現在のVaultを置き換えることを確認しました", "I confirm replacing the current Vault") }
    var importSummary: String { text("取り込み概要", "Import Summary") }
    var back: String { text("戻る", "Back") }
    var next: String { text("次へ", "Next") }
    var runImport: String { text("インポート", "Import") }
}
