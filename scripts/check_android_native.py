"""Build Android ARM64/x86_64 native SDK layers; optionally test on an adb device."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from android_tools import d8_command, run_d8

from ci_protocol import ROOT, check_protocol


def run(*arguments, env=None):
    subprocess.run(list(map(str, arguments)), cwd=ROOT, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ndk", type=Path, required=True)
    parser.add_argument("--sdk", type=Path, required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--build-tools", required=True)
    parser.add_argument("--api", type=int, default=26, help="native build API; not a supported-runtime guarantee")
    parser.add_argument("--device", help="exact adb serial; omit for compilation only")
    parser.add_argument("--r8-jar", type=Path, help="permanently installed standalone R8/D8 JAR; overrides Build-Tools D8")
    options = parser.parse_args()
    ndk = options.ndk.resolve()
    sdk = options.sdk.resolve()
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
        for language, suffix in (("c", "c"), ("cpp", "cpp")):
            driver = str(compiler) + ("++" if language == "cpp" else "")
            for name in ("test", "registry_test"):
                flags = ["-std=c++17", "-static-libstdc++"] if language == "cpp" else ["-std=c11"]
                run(driver, *flags, "-Wall", "-Wextra", "-Werror", "-pthread",
                    "-Ibindings/c", "-Ibindings/cpp", ROOT / f"bindings/{language}/{name}.{suffix}",
                    f"-L{directory}", "-lunilyrics_ffi", "-o", directory / f"{language}-{name}")
        for library in ("libunilyrics_ffi.so", "libunilyrics_jni.so"):
            headers = subprocess.check_output([tools / "llvm-readelf", "-lW", directory / library], text=True)
            loads = [line.split() for line in headers.splitlines() if line.strip().startswith("LOAD ")]
            if not loads or any(int(fields[-1], 16) < 16384 for fields in loads):
                raise AssertionError(f"Insufficient ELF load alignment: {abi}/{library}")
    print("Both Android ABIs build; engine/JNI ELF load alignments are at least 16 KB.", flush=True)
    if not options.device:
        return
    adb = sdk / "platform-tools/adb"
    command = [str(adb), "-s", options.device]
    abi = subprocess.check_output([*command, "shell", "getprop", "ro.product.cpu.abi"], text=True).strip()
    if abi not in targets:
        raise SystemExit(f"Device ABI {abi!r} is not among the built ABIs")
    run(*command, "shell", "getprop", "ro.build.version.sdk")
    run(*command, "shell", "getconf", "PAGE_SIZE")
    with tempfile.TemporaryDirectory(prefix="unilyrics-native-") as temporary:
        scratch = Path(temporary)
        classes = scratch / "classes"
        classes.mkdir()
        platform = sdk / "platforms" / options.platform
        sources = sorted((ROOT / "bindings/java/src/main/java").rglob("*.java"))
        run("javac", "--release", "17", "-classpath", platform / "android.jar", "-d", classes,
            *sources, ROOT / "bindings/java/AndroidNativeTest.java", ROOT / "bindings/java/JsonTest.java")
        run("jar", "--create", "--file", scratch / "java.jar", "-C", classes, ".")
        run("kotlinc", ROOT / "bindings/kotlin/src/main/kotlin/org/tharukrj/unilyrics/Lyrics.kt",
            ROOT / "bindings/kotlin/AndroidNativeTest.kt", "-jvm-target", "17", "-classpath", scratch / "java.jar",
            "-include-runtime", "-d", scratch / "kotlin.jar")
        dex = scratch / "dex"
        dex.mkdir()
        run_d8(d8_command(sdk, options.build_tools, options.r8_jar), "--release", "--min-api", "29",
            "--lib", platform / "android.jar", "--lib", platform / "core-for-system-modules.jar",
            "--output", dex, scratch / "java.jar", scratch / "kotlin.jar")
        remote = f"/data/local/tmp/{scratch.name}"
        run(*command, "shell", "mkdir", remote)
        try:
            for name in ("libunilyrics_ffi.so", "libunilyrics_jni.so", "c-test", "c-registry_test", "cpp-test", "cpp-registry_test"):
                run(*command, "push", output / abi / name, remote + "/" + name)
            run(*command, "push", dex / "classes.dex", remote + "/classes.dex")
            shell = [*command, "shell", "env", f"LD_LIBRARY_PATH={remote}"]
            for name in ("c-test", "c-registry_test", "cpp-test", "cpp-registry_test"):
                run(*command, "shell", "chmod", "700", remote + "/" + name)
                run(*shell, remote + "/" + name)
            java = [*shell, "dalvikvm", f"-Djava.library.path={remote}", "-cp", remote + "/classes.dex"]
            for name in ("org.tharukrj.unilyrics.JsonTest", "org.tharukrj.unilyrics.AndroidNativeTest", "AndroidNativeTestKt"):
                run(*java, name)
            check_protocol([*java, "org.tharukrj.unilyrics.AndroidNativeTest", "--protocol"])
            check_protocol([*shell, remote + "/c-test", "--protocol"])
        finally:
            run(*command, "shell", "rm", "-rf", remote)
    print(f"Android native C/C++ and Java/Kotlin JNI checks passed on {abi}.")


if __name__ == "__main__":
    main()
