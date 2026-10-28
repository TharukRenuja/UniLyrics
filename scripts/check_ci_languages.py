"""Run all 11 source SDK suites on Linux; missing tools are failures."""
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys

from ci_protocol import ROOT, check_protocol, check_cpp_registry
from host_check_log import logged_check


def check(run, directory):
    if sys.platform != "linux":
        raise RuntimeError("Run the 11-language suite on Linux")
    required = ["cargo", "rustup", "node", "tsc", "gcc", "g++", "go", "javac", "java", "kotlinc", "swift", "dotnet"]
    missing = [name for name in required if not shutil.which(name)]
    if missing:
        raise RuntimeError("Missing required SDK tools: " + ", ".join(missing))
    run(["cargo", "test", "--locked", "--offline", "--workspace"])
    run(["cargo", "build", "--locked", "--offline", "-p", "unilyrics-ffi"])
    native = ROOT / "target/debug"
    run([sys.executable, "-B", "bindings/python/test_conformance.py", native / "libunilyrics_ffi.so"])
    run([sys.executable, "-B", "bindings/c/test_profiles.py", native / "libunilyrics_ffi.so"])
    for compiler, source, name, standard in [
        ("gcc", "bindings/c/test.c", "smoke-c", "-std=c11"),
        ("gcc", "bindings/c/registry_test.c", "registry-c", "-std=c11"),
        ("g++", "bindings/cpp/test.cpp", "smoke-cpp", "-std=c++17"),
        ("g++", "bindings/cpp/registry_test.cpp", "registry-cpp", "-std=c++17"),
    ]:
        binary = ROOT / "target" / name
        run([compiler, standard, "-pthread", "-Wall", "-Wextra", "-Werror", "-Ibindings/c", source, "-L" + str(native), "-lunilyrics_ffi", "-Wl,-rpath," + str(native), "-o", binary])
        run([binary])
        check_protocol([binary, "--protocol"])
        if name == "registry-cpp":
            check_cpp_registry(binary)

    if "rustup" not in missing and "node" not in missing:
        targets = subprocess.check_output(["rustup", "target", "list", "--installed"], text=True)
        if "wasm32-unknown-unknown" in targets:
            run(["cargo", "build", "--locked", "--offline", "-p", "unilyrics-ffi", "--release", "--target", "wasm32-unknown-unknown"])
            dest = ROOT / "bindings/javascript/dist"
            dest.mkdir(exist_ok=True)
            shutil.copyfile(ROOT / "target/wasm32-unknown-unknown/release/unilyrics_ffi.wasm", dest / "core.wasm")
            run(["node", "bindings/javascript/test.mjs"])
        else:
            raise RuntimeError("Missing Rust wasm32-unknown-unknown target")
    if "tsc" not in missing:
        run(["tsc", "--noEmit", "--strict", "--target", "ES2022", "--module", "NodeNext", "--moduleResolution", "NodeNext", "bindings/javascript/typecheck.ts"])

    if "go" not in missing:
        env = os.environ.copy()
        env["CGO_ENABLED"] = "1"
        env["CGO_LDFLAGS"] = shlex.join(["-L" + str(native), "-Wl,-rpath," + str(native)])
        env.setdefault("GOCACHE", str(ROOT / "target/go-cache"))
        env["GOTOOLCHAIN"] = "local"
        run(["go", "test", "-race", "./..."], cwd=ROOT / "bindings/go", env=env)

    if "javac" not in missing and "java" not in missing:
        java_home = Path(shutil.which("javac")).resolve().parent.parent
        java_out = ROOT / "bindings/java/build"
        java_out.mkdir(exist_ok=True)
        bridge = java_out / "libunilyrics_jni.so"
        run(["gcc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror", "-I" + str(java_home / "include"), "-I" + str(java_home / "include/linux"), "bindings/java/native/unilyrics_jni.c", "-L" + str(native), "-lunilyrics_ffi", "-Wl,-rpath," + str(native), "-o", bridge])
        json_jar = Path(os.environ.get("UNILYRICS_JSON_JAR", ROOT / "target/deps/json-20250517.jar"))
        if not json_jar.is_file():
            raise SystemExit("Missing JSON-java dependency: set UNILYRICS_JSON_JAR or supply target/deps/json-20250517.jar (see bindings/java/pom.xml).")
        java_classpath = str(java_out) + os.pathsep + str(json_jar)
        java_sources = sorted((ROOT / "bindings/java/src/main/java").rglob("*.java"))
        run(["javac", "--release", "17", "-classpath", json_jar, "-d", java_out, *java_sources, "bindings/java/Test.java", "bindings/java/RegistryTest.java", "bindings/java/JsonTest.java"])
        run(["java", "-cp", java_classpath, "org.tharukrj.unilyrics.JsonTest"])
        run(["java", "-cp", java_classpath, "Test", bridge])
        run(["java", f"-Djava.library.path={native}{os.pathsep}{java_out}", "-cp", java_classpath, "Test", "--system"])
        run(["java", "-cp", java_classpath, "RegistryTest", bridge, ROOT / "crates/unilyrics/tests/fixtures/conformance.json"])
        check_protocol(["java", "-cp", java_classpath, "Test", bridge, "--protocol"])
        check_protocol(["java", "-cp", java_classpath, "Test", bridge, "--typed-protocol"])
        if "kotlinc" not in missing:
            kotlin_out = ROOT / "bindings/kotlin/build"
            kotlin_out.mkdir(exist_ok=True)
            jar = kotlin_out / "tests.jar"
            run(["kotlinc", "bindings/kotlin/src/main/kotlin/org/tharukrj/unilyrics/Lyrics.kt", "bindings/kotlin/Test.kt", "-classpath", java_classpath, "-include-runtime", "-d", jar])
            run(["java", "-cp", str(jar) + os.pathsep + java_classpath, "TestKt", bridge, ROOT / "crates/unilyrics/tests/fixtures/conformance.json"])
            run(["java", f"-Djava.library.path={native}{os.pathsep}{java_out}", "-cp", str(jar) + os.pathsep + java_classpath, "TestKt", "--system", ROOT / "crates/unilyrics/tests/fixtures/conformance.json"])
            check_protocol(["java", "-cp", str(jar) + os.pathsep + java_classpath, "TestKt", bridge, "--protocol"])
    if "swift" not in missing:
        env = os.environ.copy()
        env["CLANG_MODULE_CACHE_PATH"] = str(ROOT / "target/swift-clang-cache")
        env["SWIFTPM_MODULECACHE_OVERRIDE"] = str(ROOT / "target/swift-module-cache")
        run(["swift", "test", "--package-path", "bindings/swift",
             "--cache-path", ROOT / "target/swift-cache",
             "--config-path", ROOT / "target/swift-config",
             "--security-path", ROOT / "target/swift-security",
             "--scratch-path", ROOT / "target/swift-build",
             "-Xlinker", "-L" + str(native), "-Xlinker", "-rpath", "-Xlinker", native], env=env)

    if "dotnet" not in missing:
        run([sys.executable, "-B", "scripts/check_ci_desktop.py"])

    return "All 11 Linux source SDKs; installed packages are checked separately."


if __name__ == "__main__":
    sys.exit(logged_check("languages", check))
