"""Build local SDK artifacts for Linux x86_64. Requires Python 3.11+; never publishes."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tomllib

from package_docs import copy_examples, copy_public_docs
from build_support import ROOT, run


def main():
    if sys.platform != "linux" or platform.machine() != "x86_64":
        raise SystemExit("Local packaging currently targets Linux x86_64 only.")
    required = ["cargo", "cmake", "gcc", "javac", "jar", "kotlinc", "npm"]
    absent = [name for name in required if not shutil.which(name)]
    if absent:
        raise SystemExit("Missing packaging tools: " + ", ".join(absent))
    json_jar = Path(os.environ.get("UNILYRICS_JSON_JAR", ROOT / "target/deps/json-20250517.jar")).resolve()
    if not json_jar.is_file() or hashlib.sha256(json_jar.read_bytes()).hexdigest() != "3ea61b2a06e31edf1c91134fe9106b0ebb16628be169f3db75bc7a2b06b45796":
        raise SystemExit("Supply the pinned JSON-java jar (see docs/INSTALLATION.md); no dependencies are downloaded.")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    output = ROOT / "target/packages"
    stage = ROOT / "target/package-stage"
    # All build scratch/output is owned by this script; rebuild to avoid stale artifacts.
    for directory in [output, stage]:
        if directory.exists():
            shutil.rmtree(directory)
        directory.mkdir(parents=True)
    run(["cargo", "build", "--locked", "--offline", "--release", "-p", "unilyrics-ffi"])
    run(["cargo", "build", "--locked", "--offline", "--release", "-p", "unilyrics-ffi", "--target", "wasm32-unknown-unknown"])
    native = ROOT / "target/release"
    sdk = stage / "unilyrics-native"
    cmake = stage / "cmake"
    run(["cmake", "-S", ROOT / "bindings/c", "-B", cmake,
         "-DUNILYRICS_NATIVE_DIR=" + str(native), "-DCMAKE_INSTALL_LIBDIR=lib"])
    run(["cmake", "--install", cmake, "--prefix", sdk])
    for language in ["go", "swift", "android"]:
        shutil.copytree(ROOT / "bindings" / language, sdk / "bindings" / language,
                        ignore=shutil.ignore_patterns(".build", "build", "__pycache__"))
    copy_public_docs(ROOT, sdk)
    shutil.copytree(ROOT / "spec", sdk / "spec")
    copy_examples(ROOT, sdk)
    with tarfile.open(output / f"unilyrics-native-{version}-linux-x86_64.tar.gz", "w:gz") as archive:
        archive.add(sdk, arcname="unilyrics-native")

    python = stage / "python"
    shutil.copytree(ROOT / "bindings/python", python,
                    ignore=shutil.ignore_patterns("build", "*.egg-info", "__pycache__"))
    shutil.copytree(ROOT / "licenses", python / "licenses")
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", python)
    env = os.environ.copy()
    env["UNILYRICS_NATIVE_LIBRARY"] = str(native / "libunilyrics_ffi.so")
    run([sys.executable, "-m", "pip", "wheel", "--no-build-isolation", "--no-deps", "--no-index",
         "--wheel-dir", output, python], env=env)

    wasm = ROOT / "bindings/javascript/dist/core.wasm"
    wasm.parent.mkdir(exist_ok=True)
    shutil.copy2(ROOT / "target/wasm32-unknown-unknown/release/unilyrics_ffi.wasm", wasm)
    shutil.copytree(ROOT / "licenses", wasm.parent / "licenses", dirs_exist_ok=True)
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", wasm.parent)
    run(["npm", "pack", "--offline", "--ignore-scripts", "--cache", ROOT / "target/npm-cache", "--pack-destination", output], cwd=ROOT / "bindings/javascript")

    classes = stage / "java-classes"
    classes.mkdir()
    (classes / "META-INF").mkdir()
    shutil.copy2(ROOT / "LICENSE", classes / "META-INF/LICENSE")
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", classes / "META-INF/THIRD_PARTY_NOTICES.md")
    shutil.copytree(ROOT / "licenses", classes / "META-INF/licenses")
    jdk = Path(shutil.which("javac")).resolve().parent.parent
    resources = classes / "org/tharukrj/unilyrics/native/linux-x86_64"
    resources.mkdir(parents=True)
    shutil.copy2(native / "libunilyrics_ffi.so", resources)
    run(["gcc", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror",
         "-I" + str(jdk / "include"), "-I" + str(jdk / "include/linux"),
         ROOT / "bindings/java/native/unilyrics_jni.c", "-L" + str(native), "-lunilyrics_ffi",
         "-Wl,-rpath,$ORIGIN", "-o", resources / "libunilyrics_jni.so"])
    java_sources = sorted((ROOT / "bindings/java/src/main/java").rglob("*.java"))
    run(["javac", "--release", "17", "-classpath", json_jar, "-d", classes, *java_sources])
    run(["jar", "--create", "--file", output / f"unilyrics-{version}-linux-x86_64.jar", "-C", classes, "."])
    shutil.copy2(ROOT / "bindings/java/pom.xml", output / f"unilyrics-{version}.pom")
    shutil.copy2(json_jar, output / json_jar.name)
    run(["kotlinc", ROOT / "bindings/kotlin/src/main/kotlin/org/tharukrj/unilyrics/Lyrics.kt",
         "-jvm-target", "17", "-classpath", str(classes) + os.pathsep + str(json_jar),
         "-d", output / f"unilyrics-kotlin-{version}.jar"])
    run(["jar", "--update", "--file", output / f"unilyrics-kotlin-{version}.jar", "-C", ROOT, "LICENSE"])
    run(["cargo", "package", "--locked", "--offline", "--allow-dirty", "-p", "unilyrics"])
    shutil.copy2(ROOT / f"target/package/unilyrics-{version}.crate", output)
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", output)
    shutil.copytree(ROOT / "licenses", output / "licenses")
    artifacts = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(output.iterdir()) if path.is_file()}
    (output / "manifest.json").write_text(json.dumps({"version": version, "platform": "linux-x86_64", "build_libc": platform.libc_ver(), "sha256": artifacts}, indent=2) + "\n")
    print("Local packages built: " + str(output))


if __name__ == "__main__":
    main()
