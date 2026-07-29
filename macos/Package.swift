// swift-tools-version: 6.3
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "CodexURLVault",
    platforms: [
        .macOS(.v14)
    ],
    targets: [
        .executableTarget(
            name: "CodexURLVault",
            linkerSettings: [
                .unsafeFlags([
                    "-L", "Libraries",
                    "-lurl_vault_ffi",
                    "-lsqlite3",
                    "-framework", "Security",
                    "-framework", "SystemConfiguration"
                ])
            ]
        ),
        .testTarget(
            name: "CodexURLVaultTests",
            dependencies: ["CodexURLVault"]
        ),
    ],
    swiftLanguageModes: [.v5]
)
