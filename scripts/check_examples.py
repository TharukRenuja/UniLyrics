"""Compile/run the 11 source-tree examples offline on Linux with installed tools."""
import hashlib
from xml.sax.saxutils import escape
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from build_support import ROOT, run


def main():
    if sys.platform != "linux":
        raise SystemExit("This source example runner currently targets Linux.")
    for tool in ("cargo", "node", "tsc", "gcc", "g++", "go", "javac", "java", "kotlinc", "swift", "dotnet"):
        if not shutil.which(tool):
            raise SystemExit(f"Missing permanent tool: {tool}; nothing was installed")
    dependency = Path(os.environ.get("UNILYRICS_JSON_JAR", ROOT / "target/deps/json-20250517.jar")).resolve()
    if not dependency.is_file() or hashlib.sha256(dependency.read_bytes()).hexdigest() != "3ea61b2a06e31edf1c91134fe9106b0ebb16628be169f3db75bc7a2b06b45796":
        raise SystemExit("Supply the pinned JSON-java jar described in docs/INSTALLATION.md; no downloads occur.")
    output = ROOT / "target/examples"
    output.mkdir(exist_ok=True)
    examples = ROOT / "examples"
    native = ROOT / "target/debug"
    run(["cargo", "build", "--locked", "--offline", "-p", "unilyrics-ffi"])
    run(["cargo", "build", "--locked", "--offline", "--release", "-p", "unilyrics-ffi", "--target", "wasm32-unknown-unknown"])
    wasm = ROOT / "bindings/javascript/dist/core.wasm"
    wasm.parent.mkdir(exist_ok=True)
    shutil.copy2(ROOT / "target/wasm32-unknown-unknown/release/unilyrics_ffi.wasm", wasm)
    documents = {}

    def verify(language, arguments, cwd=ROOT, env=None):
        result = subprocess.check_output(list(map(str, arguments)), cwd=cwd, env=env, text=True)
        document = json.loads(result)
        if language in ("C", "C++"):
            assert document["ok"] is True, document
            document = document["result"]["document"]
        assert document["version"] == 1 and len(document["cues"]) == 1, (language, document)
        cue = document["cues"][0]
        assert cue["start_us"] == 1000000 and cue["end_us"] is None, (language, cue)
        assert "".join(segment["text"] for segment in cue["segments"]) == "Hello 世界 🎵", (language, cue)
        if documents:
            assert document == next(iter(documents.values())), (language, document)
        documents[language] = document
        print(f"{language} example passed.", flush=True)

    verify("JavaScript", ["node", examples / "javascript.mjs"])
    run(["tsc", "--strict", "--noEmit", "--target", "ES2022", "--module", "NodeNext", "--moduleResolution", "NodeNext", examples / "typescript.mts"])
    verify("TypeScript", ["node", "--experimental-strip-types", "--input-type=module", "-e",
        'import {readFile} from "node:fs/promises"; import {parseExample} from "./examples/typescript.mts"; console.log(JSON.stringify(await parseExample(await readFile("bindings/javascript/dist/core.wasm"))));'])
    environment = dict(os.environ, PYTHONPATH=str(ROOT / "bindings/python"),
                       UNILYRICS_LIBRARY=str(native / "libunilyrics_ffi.so"))
    verify("Python", [sys.executable, "-B", examples / "python.py"], env=environment)
    for language, compiler, source in (("C", "gcc", "c.c"), ("C++", "g++", "cpp.cpp")):
        executable = output / compiler
        run([compiler, "-std=c11" if language == "C" else "-std=c++17", "-Wall", "-Wextra", "-Werror",
             "-Ibindings/c", "-Ibindings/cpp", examples / source, f"-L{native}", "-lunilyrics_ffi",
             f"-Wl,-rpath,{native}", "-o", executable])
        verify(language, [executable])
    rust = output / "rust"
    (rust / "src").mkdir(parents=True, exist_ok=True)
    shutil.copy2(examples / "rust.rs", rust / "src/main.rs")
    (rust / "Cargo.toml").write_text('[package]\nname="unilyrics-example"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[dependencies]\nunilyrics={path=' + json.dumps(str(ROOT / "crates/unilyrics")) + '}\n')
    verify("Rust", ["cargo", "run", "--offline", "--quiet"], cwd=rust)
    go = output / "go"
    go.mkdir(exist_ok=True)
    shutil.copy2(examples / "go.go", go / "main.go")
    (go / "go.mod").write_text('module example.local/unilyrics\n\ngo 1.21\nrequire github.com/TharukRenuja/UniLyrics/bindings/go v0.0.0\nreplace github.com/TharukRenuja/UniLyrics/bindings/go => ' + json.dumps(str(ROOT / "bindings/go")) + '\n')
    environment = dict(os.environ, GOCACHE=str(ROOT / "target/go-cache"), GOTOOLCHAIN="local", GOPROXY="off",
                       CGO_ENABLED="1", CGO_LDFLAGS=f"-L{native} -Wl,-rpath,{native}")
    verify("Go", ["go", "run", "."], cwd=go, env=environment)
    java = output / "java"
    java.mkdir(exist_ok=True)
    jdk = Path(shutil.which("javac")).resolve().parent.parent
    run(["gcc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror", f"-I{jdk / 'include'}", f"-I{jdk / 'include/linux'}",
         ROOT / "bindings/java/native/unilyrics_jni.c", f"-L{native}", "-lunilyrics_ffi", f"-Wl,-rpath,{native}", "-o", java / "libunilyrics_jni.so"])
    sources = sorted((ROOT / "bindings/java/src/main/java").rglob("*.java"))
    run(["javac", "--release", "17", "-classpath", dependency, "-d", java, *sources, examples / "Example.java"])
    classpath = str(java) + os.pathsep + str(dependency)
    loader = f"-Djava.library.path={java}{os.pathsep}{native}"
    verify("Java", ["java", loader, "-cp", classpath, "Example"])
    run(["kotlinc", ROOT / "bindings/kotlin/src/main/kotlin/org/tharukrj/unilyrics/Lyrics.kt", examples / "kotlin.kt",
         "-jvm-target", "17", "-classpath", classpath, "-include-runtime", "-d", java / "example.jar"])
    verify("Kotlin", ["java", loader, "-cp", str(java / "example.jar") + os.pathsep + classpath, "KotlinKt"])
    swift = output / "swift-consumer"
    (swift / "Sources/Example").mkdir(parents=True, exist_ok=True)
    shutil.copy2(examples / "swift.swift", swift / "Sources/Example/main.swift")
    (swift / "Package.swift").write_text('// swift-tools-version: 5.9\nimport PackageDescription\nlet package = Package(name: "Example", dependencies: [.package(name: "UniLyrics", path: ' + json.dumps(str(ROOT / "bindings/swift")) + ')], targets: [.executableTarget(name: "Example", dependencies: [.product(name: "UniLyrics", package: "UniLyrics")])])\n')
    verify("Swift", ["swift", "run", "--package-path", swift, "--cache-path", ROOT / "target/swift-cache",
        "--config-path", ROOT / "target/swift-config", "--security-path", ROOT / "target/swift-security",
        "-Xlinker", f"-L{native}", "-Xlinker", "-rpath", "-Xlinker", native, "Example"])
    dotnet = output / "dotnet"
    dotnet.mkdir(exist_ok=True)
    shutil.copy2(examples / "csharp.cs", dotnet / "Program.cs")
    project = dotnet / "Example.csproj"
    reference = escape(str(ROOT / "bindings/dotnet/UniLyrics/UniLyrics.csproj"), {'"': '&quot;'})
    project.write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><ProjectReference Include="' + reference + '" /></ItemGroup></Project>')
    config = dotnet / "NuGet.Config"
    config.write_text('<configuration><packageSources><clear /></packageSources></configuration>')
    dotnet_env = dict(os.environ, DOTNET_CLI_HOME=str(ROOT / "target/dotnet-home"),
                      DOTNET_CLI_TELEMETRY_OPTOUT="1", UNILYRICS_LIBRARY=str(native / "libunilyrics_ffi.so"))
    run(["dotnet", "restore", project, "--configfile", config], env=dotnet_env)
    run(["dotnet", "build", project, "--no-restore", "-c", "Release"], env=dotnet_env)
    verify("C#", ["dotnet", dotnet / "bin/Release/net10.0/Example.dll"], env=dotnet_env)
    assert len(documents) == 11
    (output / "result.json").write_text(json.dumps(documents, ensure_ascii=False, indent=2) + "\n")
    print("All 11 examples produce the same canonical document.")


if __name__ == "__main__":
    main()
