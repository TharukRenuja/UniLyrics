"""Build and execute Rust, C/C++ and Swift checks on the current Mac architecture."""
import os
import sys
from host_check_log import ROOT, logged_check


def check(run, directory):
    if sys.platform != 'darwin':
        raise RuntimeError('Run this script on macOS with Xcode selected.')
    run(['xcodebuild', '-version'])
    run(['xcrun', '--sdk', 'macosx', '--show-sdk-path'])
    run(['cargo', 'test', '--locked', '--offline', '--workspace'])
    run(['cargo', 'build', '--locked', '--offline', '--release', '-p', 'unilyrics-ffi'])
    native = ROOT / 'target/release'
    for compiler, extension, standard in [('clang', 'c', 'c11'), ('clang++', 'cpp', 'c++17')]:
        for name in ('test', 'registry_test'):
            binary = directory / f'{extension}-{name}'
            run(['xcrun', compiler, f'-std={standard}', '-Wall', '-Wextra', '-Werror',
                 '-Ibindings/c', '-Ibindings/cpp', f'bindings/{extension}/{name}.{extension}',
                 f'-L{native}', '-lunilyrics_ffi', f'-Wl,-rpath,{native}', '-o', binary])
            run([binary])
    run(['swift', 'test', '--package-path', ROOT / 'bindings/swift',
         '--scratch-path', directory / 'swift-build', '-Xlinker', f'-L{native}',
         '-Xlinker', '-rpath', '-Xlinker', native], env=os.environ.copy())
    return 'Current-architecture macOS Rust, C/C++ and Swift tests; iOS and XCFramework packaging are not checked.'


if __name__ == '__main__':
    sys.exit(logged_check('macos', check))
