"""Build an XCFramework and exercise a separate Swift package on macOS and iOS Simulator."""
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys

from host_check_log import ROOT, logged_check


def prepare_consumer(directory, framework):
    package = directory / 'consumer'
    package.mkdir()
    shutil.copytree(framework, package / 'CUniLyrics.xcframework')
    shutil.copytree(ROOT / 'bindings/swift/Sources/UniLyrics', package / 'Sources/UniLyrics')
    tests = package / 'Tests/UniLyricsTests'
    shutil.copytree(ROOT / 'bindings/swift/Tests/UniLyricsTests', tests)
    shutil.copytree(ROOT / 'crates/unilyrics/tests/fixtures', tests / 'Fixtures')
    for source in tests.glob('*.swift'):
        text = source.read_text()
        text = re.sub(r'root.appendingPathComponent\("crates/unilyrics/tests/fixtures/([^".]+)\.json"\)',
                      r'Bundle.module.url(forResource: "\1", withExtension: "json")!', text)
        text = text.replace('        var root = URL(fileURLWithPath: #filePath)\n        for _ in 0..<5 { root.deleteLastPathComponent() }\n', '')
        source.write_text(text)
    (package / 'Package.swift').write_text('''// swift-tools-version: 5.9
import PackageDescription
let package = Package(name: "UniLyrics", platforms: [.macOS(.v13), .iOS(.v15)],
    products: [.library(name: "UniLyrics", targets: ["UniLyrics"])],
    targets: [
        .binaryTarget(name: "CUniLyrics", path: "CUniLyrics.xcframework"),
        .target(name: "UniLyrics", dependencies: ["CUniLyrics"]),
        .testTarget(name: "UniLyricsTests", dependencies: ["UniLyrics"], resources: [.process("Fixtures")])
    ])
''')
    return package


def simulator_id(devices):
    for runtime, entries in devices['devices'].items():
        if '.iOS-' in runtime:
            for device in entries:
                if device.get('isAvailable') and device['name'].startswith('iPhone'):
                    return device['udid']
    raise RuntimeError('No available iPhone simulator; install an iOS simulator runtime in Xcode')


def check(run, directory):
    if sys.platform != 'darwin':
        raise RuntimeError('Run on a Mac with Xcode and an iOS simulator runtime')
    host = {'arm64': 'aarch64-apple-darwin', 'x86_64': 'x86_64-apple-darwin'}[platform.machine()]
    simulator = 'aarch64-apple-ios-sim' if host.startswith('aarch64') else 'x86_64-apple-ios'
    targets = [host, 'aarch64-apple-ios', simulator]
    headers = directory / 'headers'
    headers.mkdir()
    shutil.copy2(ROOT / 'bindings/c/unilyrics.h', headers)
    (headers / 'module.modulemap').write_text('module CUniLyrics { header "unilyrics.h" export * }\n')
    framework = directory / 'CUniLyrics.xcframework'
    command = ['xcodebuild', '-create-xcframework']
    environment = dict(os.environ, MACOSX_DEPLOYMENT_TARGET='13.0', IPHONEOS_DEPLOYMENT_TARGET='15.0')
    for target in targets:
        run(['cargo', 'build', '--locked', '--offline', '--release', '-p', 'unilyrics-ffi', '--target', target], env=environment)
        command += ['-library', ROOT / 'target' / target / 'release/libunilyrics_ffi.a', '-headers', headers]
    run(command + ['-output', framework])
    package = prepare_consumer(directory, framework)
    run(['swift', 'test', '--package-path', package])
    devices = json.loads(subprocess.check_output(['xcrun', 'simctl', 'list', 'devices', 'available', '--json'], text=True))
    device = simulator_id(devices)
    run(['xcodebuild', '-scheme', 'UniLyrics', '-destination', 'id=' + device,
         '-derivedDataPath', directory / 'DerivedData', '-resultBundlePath', directory / 'iOS.xcresult',
         'CODE_SIGNING_ALLOWED=NO', 'test'], cwd=package)
    return 'XCFramework consumed by separate Swift package on this Mac and iOS Simulator; physical iOS devices and deployment minimums are not verified.'


if __name__ == '__main__':
    sys.exit(logged_check('apple-package', check))
