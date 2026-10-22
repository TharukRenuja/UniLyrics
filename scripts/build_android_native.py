"""Build Android ARM64/x86_64 native libraries using the Linux NDK."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

from build_support import ROOT


def run(*arguments, env=None):
    subprocess.run(list(map(str, arguments)), cwd=ROOT, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ndk", type=Path, required=True)
    parser.add_argument("--api", type=int, default=29, help="native build API, minimum 29")
    options = parser.parse_args()
    if options.api < 29:
        raise SystemExit("Android minimum API is 29")
    ndk = options.ndk.resolve()
    tools = ndk / "toolchains/llvm/prebuilt/linux-x86_64/bin"
    targets = {"arm64-v8a": "aarch64-linux-android", "x86_64": "x86_64-linux-android"}
    environment = os.environ.copy()
    for target in targets.values():
        compiler = tools / f"{target}{options.api}-clang"
        if not compiler.is_file():
            raise SystemExit(f"Missing installed NDK compiler: {compiler}")
        environment[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(compiler)
    run("cargo", "build", "--locked", "--offline", "--release", "-p", "unilyrics-ffi",
        *[argument for target in targets.values() for argument in ("--target", target)], env=environment)
    output = ROOT / "target/android-native"
    for abi, target in targets.items():
        directory = output / abi
        directory.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "target" / target / "release/libunilyrics_ffi.so", directory)
        compiler = tools / f"{target}{options.api}-clang"
        run(compiler, "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
            ROOT / "bindings/java/native/unilyrics_jni.c", f"-L{directory}", "-lunilyrics_ffi",
            "-Wl,-soname,libunilyrics_jni.so", "-o", directory / "libunilyrics_jni.so")
        for library in ("libunilyrics_ffi.so", "libunilyrics_jni.so"):
            headers = subprocess.check_output([tools / "llvm-readelf", "-lW", directory / library], text=True)
            loads = [line.split() for line in headers.splitlines() if line.strip().startswith("LOAD ")]
            if not loads or any(int(fields[-1], 16) < 16384 for fields in loads):
                raise AssertionError(f"Insufficient ELF load alignment: {abi}/{library}")
    print("Android ARM64/x86_64 native libraries built with 16 KB ELF alignment.")


if __name__ == "__main__":
    main()
