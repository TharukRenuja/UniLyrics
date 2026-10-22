"""Build/check a local Android AAR using installed SDK/NDK tools; never publishes."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import zipfile
import xml.etree.ElementTree as ET

from android_tools import d8_command, run_d8

from build_support import ROOT, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ndk", type=Path, required=True)
    parser.add_argument("--sdk", type=Path, required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--build-tools", required=True)
    parser.add_argument("--r8-jar", type=Path, help="permanently installed standalone R8/D8 JAR; overrides Build-Tools D8")
    options = parser.parse_args()
    sdk = options.sdk.resolve()
    platform = sdk / "platforms" / options.platform
    compiler_command = d8_command(sdk, options.build_tools, options.r8_jar)
    compiler_version = subprocess.check_output([*compiler_command, "--version"], text=True).strip()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    kotlin_tool = shutil.which("kotlinc")
    if not kotlin_tool:
        raise SystemExit("Missing permanent Kotlin compiler; nothing was installed")
    kotlin_version = subprocess.check_output([kotlin_tool, "-version"], stderr=subprocess.STDOUT, text=True)
    match = re.search(r"kotlinc-jvm ([0-9]+\.[0-9]+\.[0-9]+)\b", kotlin_version)
    if not match:
        raise SystemExit("Cannot identify installed Kotlin compiler version")
    kotlin_version = match.group(1)
    stdlib = Path(kotlin_tool).resolve().parent.parent / "lib/kotlin-stdlib.jar"
    annotations = stdlib.with_name("annotations-13.0.jar")
    if not annotations.is_file():
        raise SystemExit(f"Missing installed Kotlin annotation dependency: {annotations}")
    run([sys.executable, "-B", ROOT / "scripts/build_android_native.py", "--ndk", options.ndk])
    output = ROOT / "target/packages-android"
    output.mkdir(exist_ok=True)
    archive_name = f"unilyrics-android-{version}.aar"
    with tempfile.TemporaryDirectory(prefix="unilyrics-aar-") as temporary:
        scratch = Path(temporary)
        archive = scratch / archive_name
        classes = scratch / "classes"
        classes.mkdir()
        sources = sorted((ROOT / "bindings/java/src/main/java").rglob("*.java"))
        run(["javac", "--release", "17", "-classpath", platform / "android.jar", "-d", classes, *sources])
        run([kotlin_tool, ROOT / "bindings/kotlin/src/main/kotlin/org/tharukrj/unilyrics/Lyrics.kt",
             "-jvm-target", "17", "-classpath", classes, "-d", classes])
        jar = scratch / "classes.jar"
        run(["jar", "--create", "--file", jar, "-C", classes, "."])
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            bundle.write(ROOT / "bindings/android/AndroidManifest.xml", "AndroidManifest.xml")
            bundle.write(ROOT / "bindings/android/consumer-rules.pro", "proguard.txt")
            bundle.write(jar, "classes.jar")
            bundle.write(ROOT / "LICENSE", "META-INF/LICENSE")
            bundle.write(ROOT / "THIRD_PARTY_NOTICES.md", "META-INF/THIRD_PARTY_NOTICES.md")
            for notice in (ROOT / "licenses").rglob("*"):
                if notice.is_file():
                    bundle.write(notice, "META-INF/licenses/" + str(notice.relative_to(ROOT / "licenses")))
            for abi in ("arm64-v8a", "x86_64"):
                for name in ("libunilyrics_ffi.so", "libunilyrics_jni.so"):
                    bundle.write(ROOT / "target/android-native" / abi / name, f"jni/{abi}/{name}")
        # Compile consumers from the actual archive, never from source SDK classes.
        with zipfile.ZipFile(archive) as bundle:
            manifest = ET.fromstring(bundle.read("AndroidManifest.xml"))
            assert manifest.find("uses-sdk").get("{http://schemas.android.com/apk/res/android}minSdkVersion") == "29"
            assert b"native <methods>" in bundle.read("proguard.txt")
            jar.write_bytes(bundle.read("classes.jar"))
            with zipfile.ZipFile(io.BytesIO(jar.read_bytes())) as contents:
                names = contents.namelist()
                assert "org/tharukrj/unilyrics/UniLyrics.class" in names
                assert "org/tharukrj/unilyrics/Lyrics.class" in names
                assert not any(name.startswith(("org/json/", "kotlin/")) or "Test" in name for name in names)
            for abi in ("arm64-v8a", "x86_64"):
                for name in ("libunilyrics_ffi.so", "libunilyrics_jni.so"):
                    assert bundle.read(f"jni/{abi}/{name}") == (ROOT / "target/android-native" / abi / name).read_bytes()
        dex = scratch / "dex"
        dex.mkdir()
        run_d8(compiler_command, "--release", "--min-api", "29",
             "--lib", platform / "android.jar", "--lib", platform / "core-for-system-modules.jar",
             "--output", dex, jar, stdlib, annotations)
        assert (dex / "classes.dex").read_bytes().startswith(b"dex\n")
        shutil.copy2(archive, output / archive_name)
    archive = output / archive_name
    pom = output / f"unilyrics-android-{version}.pom"
    pom.write_text(f'''<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion><groupId>org.tharukrj.unilyrics</groupId>
  <artifactId>unilyrics-android</artifactId><version>{version}</version><packaging>aar</packaging>
  <licenses><license><name>MIT License</name><url>https://opensource.org/license/mit</url></license></licenses>
  <dependencies><dependency><groupId>org.jetbrains.kotlin</groupId><artifactId>kotlin-stdlib</artifactId>
    <version>{kotlin_version}</version></dependency>
    <dependency><groupId>org.jetbrains</groupId><artifactId>annotations</artifactId><version>13.0</version></dependency>
  </dependencies>
</project>
''')
    local_stdlib = output / f"kotlin-stdlib-{kotlin_version}.jar"
    shutil.copy2(stdlib, local_stdlib)
    local_annotations = output / annotations.name
    shutil.copy2(annotations, local_annotations)
    artifacts = [archive, pom, local_stdlib, local_annotations]
    (output / "manifest.json").write_text(json.dumps({"version": version, "min_sdk": 29,
        "abis": ["arm64-v8a", "x86_64"], "kotlin": kotlin_version,
        "ndk": options.ndk.resolve().name,
        "dex_compiler": compiler_version,
        "r8_sha256": hashlib.sha256(options.r8_jar.read_bytes()).hexdigest() if options.r8_jar else None,
        "sha256": {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in artifacts}}, indent=2) + "\n")
    print(f"Android AAR built: {archive}")
    print("Nothing published.")


if __name__ == "__main__":
    main()
