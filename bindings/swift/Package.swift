// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "UniLyrics",
    products: [.library(name: "UniLyrics", targets: ["UniLyrics"])],
    targets: [
        .systemLibrary(name: "CUniLyrics", path: "Sources/CUniLyrics"),
        .target(name: "UniLyrics", dependencies: ["CUniLyrics"], linkerSettings: [.linkedLibrary("unilyrics_ffi")]),
        .testTarget(name: "UniLyricsTests", dependencies: ["UniLyrics"])
    ]
)
