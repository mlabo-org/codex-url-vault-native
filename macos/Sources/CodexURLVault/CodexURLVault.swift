import SwiftUI

@main
struct CodexURLVaultApp: App {
    var body: some Scene {
        WindowGroup {
            AppShellRoot {
                VaultView()
            }
        }
        .commands {
            AppShellSettingsMenu()
        }

        Settings {
            AppShellRoot {
                AppShellPreferencesView()
            }
        }
    }
}
