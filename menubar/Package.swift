// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SauronMenu",
    platforms: [
        .macOS(.v13)
    ],
    targets: [
        .executableTarget(
            name: "SauronMenu",
            path: "Sources/SauronMenu",
            linkerSettings: [
                .linkedLibrary("sqlite3")
            ]
        )
    ]
)
