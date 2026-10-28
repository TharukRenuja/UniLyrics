"""Install an extracted-AAR test app on one adb device; always uninstall afterward."""
import argparse
import hashlib
import io
import json
import re
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid
import zipfile

from android_tools import d8_command, run_d8

ROOT = Path(__file__).resolve().parents[1]
ACTIVITY = "org.tharukrj.unilyrics.check.CheckActivity"


def run(*arguments, capture=False):
    return subprocess.run(list(map(str, arguments)), check=True, text=True,
                          capture_output=capture, timeout=180)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", type=Path, required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--build-tools", required=True)
    parser.add_argument("--r8-jar", type=Path, required=True)
    parser.add_argument("--aar", type=Path, required=True)
    parser.add_argument("--device", required=True)
    parser.add_argument("--shrink", action="store_true", help="run R8 optimization/shrinking instead of D8")
    options = parser.parse_args()
    sdk = options.sdk.resolve()
    tools = sdk / "build-tools" / options.build_tools
    platform = sdk / "platforms" / options.platform
    target_api = int(re.search(r"AndroidVersion.ApiLevel\s*=\s*(\d+)",
                               (platform / "source.properties").read_text()).group(1))
    kotlin = shutil.which("kotlinc")
    if not kotlin:
        raise SystemExit("Missing permanent Kotlin compiler; nothing installed")
    stdlib = Path(kotlin).resolve().parent.parent / "lib/kotlin-stdlib.jar"
    annotations = stdlib.with_name("annotations-13.0.jar")
    if not annotations.is_file():
        raise SystemExit(f"Missing installed Kotlin annotation dependency: {annotations}")
    command = d8_command(sdk, options.build_tools, options.r8_jar)
    if options.shrink:
        command[-1] = "com.android.tools.r8.R8"
    adb = [str(sdk / "platform-tools/adb"), "-s", options.device]
    api = int(run(*adb, "shell", "getprop", "ro.build.version.sdk", capture=True).stdout.strip())
    abi = run(*adb, "shell", "getprop", "ro.product.cpu.abi", capture=True).stdout.strip()
    if api < 29 or abi not in ("x86_64", "arm64-v8a"):
        raise SystemExit(f"Unsupported test device API/ABI: {api}/{abi}")
    page_size = int(run(*adb, "shell", "getconf", "PAGE_SIZE", capture=True).stdout.strip())
    aar_bytes = options.aar.read_bytes()
    example = ROOT / "examples/android/app/src/main"
    java_examples = sorted((example / "java").rglob("*.java"))
    kotlin_examples = sorted((example / "kotlin").rglob("*.kt"))
    package = "org.tharukrj.unilyrics.check.p" + uuid.uuid4().hex
    with tempfile.TemporaryDirectory(prefix="unilyrics-android-app-") as temporary:
        scratch = Path(temporary)
        jar = scratch / "sdk.jar"
        with zipfile.ZipFile(io.BytesIO(aar_bytes)) as archive:
            jar.write_bytes(archive.read("classes.jar"))
            rules = archive.read("proguard.txt").decode()
            libraries = {name: archive.read(f"jni/{abi}/{name}")
                         for name in ("libunilyrics_ffi.so", "libunilyrics_jni.so")}
        source = scratch / "CheckActivity.java"
        source.write_text('''package org.tharukrj.unilyrics.check;
import android.app.Activity;
import android.os.Bundle;
import java.io.*;
import org.tharukrj.unilyrics.AndroidNativeTest;
import org.tharukrj.unilyrics.JsonTest;
import org.tharukrj.unilyrics.AndroidFixtureTest;
import org.tharukrj.unilyrics.example.JavaExample;
import org.tharukrj.unilyrics.example.KotlinExample;
public final class CheckActivity extends Activity {
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        new Thread(() -> {
            String result;
            try {
                if (!JavaExample.run().equals("Hello 世界 🎵")) throw new AssertionError("Java example");
                if (!KotlinExample.run().equals("Hello 世界 🎵")) throw new AssertionError("Kotlin example");
                JsonTest.main(new String[0]);
                AndroidNativeTest.main(new String[0]);
                Class.forName("AndroidNativeTestKt").getMethod("main").invoke(null);
                for (String kind : new String[]{"conformance", "normalization", "profiles"}) {
                    AndroidFixtureTest.run(kind, getAssets().open(kind + ".json"));
                }
                result = "PASS\\n";
            } catch (Throwable error) {
                StringWriter details = new StringWriter();
                error.printStackTrace(new PrintWriter(details));
                result = "FAIL\\n" + details;
            }
            try (FileOutputStream output = openFileOutput("result.txt", MODE_PRIVATE)) {
                output.write(result.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            } catch (IOException error) { throw new RuntimeException(error); }
        }).start();
    }
}
''')
        kotlin_tests = scratch / "kotlin-tests.jar"
        run(kotlin, ROOT / "bindings/kotlin/AndroidNativeTest.kt", *kotlin_examples, "-jvm-target", "17",
            "-classpath", jar, "-d", kotlin_tests)
        classes = scratch / "classes"
        classes.mkdir()
        classpath = ":".join(map(str, (jar, platform / "android.jar", kotlin_tests, stdlib, annotations)))
        run("javac", "--release", "17", "-classpath", classpath,
            "-d", classes, source, *java_examples, ROOT / "bindings/java/AndroidNativeTest.java",
            ROOT / "bindings/java/JsonTest.java", ROOT / "bindings/java/AndroidFixtureTest.java")
        tests = scratch / "tests.jar"
        run("jar", "--create", "--file", tests, "-C", classes, ".")
        dex = scratch / "dex"
        dex.mkdir()
        arguments = ["--release", "--min-api", "29", "--lib", platform / "android.jar",
                     "--lib", platform / "core-for-system-modules.jar", "--output", dex]
        if options.shrink:
            config = scratch / "consumer-rules.pro"
            config.write_text(rules + '\n-keep class ' + ACTIVITY + ' { *; }\n'
                              '-keep class AndroidNativeTestKt { public static void main(...); }\n')
            arguments.extend(["--pg-conf", config])
        run_d8(command, *arguments, jar, tests, kotlin_tests, stdlib, annotations)
        manifest = scratch / "AndroidManifest.xml"
        manifest.write_text(f'''<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{package}">
  <uses-sdk android:minSdkVersion="29" android:targetSdkVersion="{target_api}"/>
  <application android:debuggable="true" android:extractNativeLibs="true" android:hasCode="true">
    <activity android:name="{ACTIVITY}" android:exported="true"/>
  </application>
</manifest>''')
        apk = scratch / "unsigned.apk"
        run(tools / "aapt2", "link", "--manifest", manifest, "-I", platform / "android.jar", "-o", apk)
        with zipfile.ZipFile(apk, "a", compression=zipfile.ZIP_DEFLATED) as archive:
            for path in dex.glob("classes*.dex"):
                archive.write(path, path.name)
            for name in ("conformance", "normalization", "profiles"):
                archive.write(ROOT / f"crates/unilyrics/tests/fixtures/{name}.json", f"assets/{name}.json")
            for name, content in libraries.items():
                archive.writestr(f"lib/{abi}/{name}", content)
        aligned = scratch / "aligned.apk"
        run(tools / "zipalign", "-f", "4", apk, aligned)
        key = scratch / "test.p12"
        run("keytool", "-genkeypair", "-keystore", key, "-storepass", "unilyrics-test",
            "-keypass", "unilyrics-test", "-alias", "test", "-keyalg", "RSA", "-validity", "1",
            "-dname", "CN=UniLyrics local test", capture=True)
        signed = scratch / "test.apk"
        run(tools / "apksigner", "sign", "--ks", key, "--ks-pass", "pass:unilyrics-test",
            "--out", signed, aligned)
        run(tools / "apksigner", "verify", signed)
        try:
            run(*adb, "install", "--no-streaming", signed)
            run(*adb, "shell", "am", "start", "-W", "-n", f"{package}/{ACTIVITY}")
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                reply = subprocess.run([*adb, "exec-out", "run-as", package, "cat", "files/result.txt"],
                                       text=True, capture_output=True, timeout=15)
                # Some adb versions return zero even when run-as cat reports a missing file.
                if reply.returncode == 0 and reply.stdout.startswith(("PASS\n", "FAIL\n")):
                    if reply.stdout != "PASS\n":
                        raise AssertionError(reply.stdout)
                    evidence = {"api": api, "target_sdk": target_api, "abi": abi, "page_size": page_size,
                                "shrink": options.shrink, "aar_sha256": hashlib.sha256(aar_bytes).hexdigest(),
                                "r8_sha256": hashlib.sha256(options.r8_jar.read_bytes()).hexdigest(),
                                "example_sources_sha256": {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                                                           for p in [*java_examples, *kotlin_examples]},
                                "fixtures": {name: len(json.loads((ROOT / f"crates/unilyrics/tests/fixtures/{name}.json").read_text()))
                                             for name in ("conformance", "normalization", "profiles")}}
                    report = ROOT / f"target/android-app-api{api}-{abi}-{'r8' if options.shrink else 'd8'}.json"
                    report.parent.mkdir(parents=True, exist_ok=True)
                    report.write_text(json.dumps(evidence, indent=2) + "\n")
                    print(f"Extracted AAR installed-app Java/Kotlin JNI tests passed: API {api}, {abi}, {page_size}-byte pages, shrink={options.shrink}")
                    break
                time.sleep(1)
            else:
                raise AssertionError("Test app did not produce a result within 60 seconds: " + reply.stdout + reply.stderr)
        finally:
            run(*adb, "uninstall", package)


if __name__ == "__main__":
    main()
