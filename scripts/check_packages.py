"""Install and exercise local package artifacts as external consumers (Linux x86_64).

Run package.py first. Requires every language toolchain; performs no downloads.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import zipfile

from build_support import ROOT, run


def write(directory, name, text):
    path = directory / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return path


def main():
    required = ["cargo", "cmake", "gcc", "g++", "node", "npm", "tsc", "go", "javac", "java", "kotlinc", "swift"]
    absent = [name for name in required if not shutil.which(name)]
    if absent:
        raise SystemExit("Missing consumer toolchains: " + ", ".join(absent))
    packages = ROOT / "target/packages"
    manifest = json.loads((packages / "manifest.json").read_text())
    for name, digest in manifest["sha256"].items():
        assert hashlib.sha256((packages / name).read_bytes()).hexdigest() == digest, name
    version = manifest["version"]
    work = Path(tempfile.mkdtemp(prefix="unilyrics-consumers-"))
    (ROOT / "target/package-consumer-location.txt").write_text(str(work) + "\n")
    print("External consumer workspace: " + str(work), flush=True)
    with tarfile.open(packages / f"unilyrics-native-{version}-linux-x86_64.tar.gz") as archive:
        archive.extractall(work, filter="data")
    sdk = work / "unilyrics-native"
    for path in (ROOT / "examples").iterdir():
        if path.is_file():
            assert (sdk / "examples" / path.name).read_bytes() == path.read_bytes(), path.name
    assert (sdk / "bindings/android/README.md").read_bytes() == (ROOT / "bindings/android/README.md").read_bytes()
    env = os.environ.copy()
    for key in ["PYTHONPATH", "UNILYRICS_LIBRARY", "LD_LIBRARY_PATH"]:
        env.pop(key, None)

    assert (sdk / "docs/NORMALIZATION.md").is_file(), "Native SDK must include normalization documentation"
    assert (sdk / "docs/EXTENSIONS.md").is_file(), "Native SDK must include extension documentation"
    assert (sdk / "docs/PRESERVATION.md").is_file(), "Native SDK must include preservation documentation"

    python = work / "python"
    run([sys.executable, "-m", "venv", "--without-pip", python])
    executable = python / "bin/python"
    run([sys.executable, "-m", "pip", "--python", executable, "install", "--no-index", "--no-deps",
         packages / f"unilyrics-{version}-py3-none-linux_x86_64.whl"], cwd=work, env=env)
    script = write(work, "python_consumer.py", '''from importlib.resources import files
import json
from typing import get_type_hints
from unilyrics import UniLyrics, UniLyricsError, Registry, Document, ParseResult, Features
assert files("unilyrics").joinpath("py.typed").is_file()
assert get_type_hints(UniLyrics.parse)["return"] is ParseResult
assert get_type_hints(UniLyrics.inspect)["return"] is Features
assert Document.__required_keys__ == {"version", "cues", "metadata", "attributes"}
sdk = UniLyrics()
assert "site-packages" in sdk._library._name
xml = "<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s' end='3s'><span begin='250ms' end='1s'>voice</span></p></body></tt>"
ttml = sdk.parse(xml, "ttml")["document"]
assert ttml["cues"][0]["segments"][0]["start_us"] == 1250000
assert sdk.parse(sdk.export(ttml, "ttml", "strict")["output"], "ttml")["document"] == ttml
assert sdk.detect("[00:01]hello") == "lrc"
parsed = sdk.parse("[00:01.25]සිංහල 👩🏽‍💻", "lrc")
assert parsed["document"]["cues"][0]["start_us"] == 1250000
bundle = sdk.export_preserved(parsed["document"], "plain", {"profile":"plain"})
assert bundle["losses"]
assert sdk.restore_preserved(bundle["output"], "plain", bundle["sidecar"])["document"] == parsed["document"]
profiled = sdk.project(parsed["document"], {"profile":"plain"})
assert sdk.export_profile(parsed["document"], "json", {"profile":"plain"})["losses"] == profiled["diagnostics"]
assert profiled["document"]["cues"][0]["start_us"] is None
assert any(d["code"] == "PROFILE_LINE_TIMING_DROPPED" for d in profiled["diagnostics"])

size_doc = sdk.parse("x", "plain")["document"]
seed = sdk.export_preserved(size_doc, "plain")
request = {"op":"restore_preserved", "input":seed["output"], "format":"plain", "sidecar":seed["sidecar"]}
size_doc["cues"][0]["segments"][0]["text"] = "x" * (1 + (1048576 - len(json.dumps(request, separators=(",", ":"), ensure_ascii=False).encode())) // 3)
try:
    sdk.export_preserved(size_doc, "plain")
    raise AssertionError("oversized restore bundle returned")
except UniLyricsError as error:
    assert error.code == "INPUT_TOO_LARGE"
blank = sdk.parse("", "plain")["document"]
assert any(d["code"] == "SEGMENT_STRUCTURE_CHANGED" for d in sdk.export(blank, "enhanced_lrc")["losses"])
try:
    sdk.export(blank, "enhanced_lrc", "strict")
    raise AssertionError("strict accepted empty-segment loss")
except UniLyricsError as error:
    assert error.code == "LOSSY_CONVERSION"
sdk.validate(parsed["document"])
assert sdk.inspect(parsed["document"])["line_timing"] is True
shifted = sdk.normalize(parsed["document"], -250000)
assert shifted["document"]["cues"][0]["start_us"] == 1000000
assert parsed["document"]["cues"][0]["start_us"] == 1250000
parsed["document"]["attributes"]["nullable"] = None
encoded = sdk.export(parsed["document"], "json", "strict")
assert sdk.parse(encoded["output"], "json")["document"] == parsed["document"]
assert sdk.export(parsed["document"], "plain")["output"] == "සිංහල 👩🏽‍💻"
try:
    sdk.parse("[00:60]bad", "lrc")
    raise AssertionError("missing error")
except UniLyricsError as error:
    assert error.code == "INVALID_TIMESTAMP"
try:
    sdk.process({"op":"parse", "input":float("nan")})
    raise AssertionError("NaN was accepted")
except ValueError:
    pass
registry = Registry(sdk)
def parse_custom(text):
    return sdk.parse(text[len("CUSTOM:"):], "plain")
def export_custom(document, policy):
    result = sdk.export(document, "plain", policy)
    result["output"] = "CUSTOM:" + result["output"]
    return result
registry.register("x-custom", parser=parse_custom, exporter=export_custom, detector=lambda text: text.startswith("CUSTOM:"))
custom = registry.parse("CUSTOM:සිංහල 👩🏽‍💻")
assert registry.export(custom["document"], "x-custom", "strict")["output"] == "CUSTOM:සිංහල 👩🏽‍💻"
assert Registry(sdk).detect("hello") == "plain"
''')
    run([executable, "-I", script], cwd=work, env=env)

    javascript = work / "javascript"
    javascript.mkdir()
    write(javascript, "package.json", '{"private":true,"type":"module"}')
    run(["npm", "install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--cache", ROOT / "target/npm-cache",
         packages / f"unilyrics-{version}.tgz"], cwd=javascript, env=env)
    script = write(javascript, "consumer.mjs", '''import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {initialize, Registry} from "unilyrics";
const sdk = await initialize(await readFile(new URL(import.meta.resolve("unilyrics/core.wasm"))));
const xml = "<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s' end='3s'><span begin='250ms' end='1s'>voice</span></p></body></tt>";
const ttml = sdk.parse(xml, "ttml").document;
assert.equal(ttml.cues[0].segments[0].start_us, 1250000);
assert.deepEqual(sdk.parse(sdk.export(ttml, "ttml", "strict").output, "ttml").document, ttml);
assert.equal(sdk.detect("[00:01]hello"), "lrc");
const {document} = sdk.parse("[00:01.25]සිංහල 👩🏽‍💻", "lrc");
assert.equal(document.cues[0].start_us, 1250000);
const bundle = sdk.exportPreserved(document, "plain", {profile:"plain"});
assert(bundle.losses.length > 0);
assert.deepEqual(sdk.restorePreserved(bundle.output, "plain", bundle.sidecar).document, document);
const sizeDoc = sdk.parse("x".repeat(400000), "plain").document;
assert.throws(() => sdk.exportPreserved(sizeDoc, "plain"), {code:"INPUT_TOO_LARGE"});
const blank = sdk.parse("", "plain").document;
assert(sdk.export(blank, "enhanced_lrc").losses.some(d => d.code === "SEGMENT_STRUCTURE_CHANGED"));
assert.throws(() => sdk.export(blank, "enhanced_lrc", "strict"), {code:"LOSSY_CONVERSION"});
const profiled = sdk.project(document, {profile:"plain"});
assert.deepEqual(sdk.exportProfile(document, "json", {profile:"plain"}).losses, profiled.diagnostics);
assert.equal(profiled.document.cues[0].start_us, null);
assert(profiled.diagnostics.some(d => d.code === "PROFILE_LINE_TIMING_DROPPED"));

sdk.validate(document);
assert.equal(sdk.inspect(document).line_timing, true);
const shifted = sdk.normalize(document, -250000);
assert.equal(shifted.document.cues[0].start_us, 1000000);
assert.equal(document.cues[0].start_us, 1250000);
document.attributes.nullable = null;
assert.deepEqual(sdk.parse(sdk.export(document, "json", "strict").output, "json").document, document);
assert.equal(sdk.export(document, "plain").output, "සිංහල 👩🏽‍💻");
assert.throws(() => sdk.parse("[00:60]bad", "lrc"), {code:"INVALID_TIMESTAMP"});
const registry = new Registry(sdk);
registry.register("x-custom", {
  matches: text => text.startsWith("CUSTOM:"),
  parse: text => sdk.parse(text.slice("CUSTOM:".length), "plain"),
  export(document, policy) {
    const result = sdk.export(document, "plain", policy);
    return {...result, output: "CUSTOM:" + result.output};
  },
});
const custom = registry.parse("CUSTOM:සිංහල 👩🏽‍💻");
assert.equal(registry.export(custom.document, "x-custom", "strict").output, "CUSTOM:සිංහල 👩🏽‍💻");
assert.equal(new Registry(sdk).detect("hello"), "plain");

''')
    run(["node", script], cwd=javascript, env=env)
    types = write(javascript, "consumer.ts", '''import {initialize, Registry, type Document, type CustomFormat} from "unilyrics";
declare const bytes: Uint8Array;
const sdk = await initialize(bytes);
const document: Document = sdk.parse("hello", "plain").document;
sdk.validate(document);
sdk.export(document, "json", "strict");
sdk.parse("lyrics", "ttml");
sdk.export(document, "ttml_lyrics", "strict");
const hasSingers: boolean = sdk.inspect(document).agents;
const registry = new Registry(sdk);
const adapter: CustomFormat = {parse: input => sdk.parse(input, "plain"), export: (document, policy) => sdk.export(document, "plain", policy)};
registry.register("x-custom", adapter);
const custom: Document = registry.parse("hello", "x-custom").document;
registry.export(custom, "x-custom", "strict");
// @ts-expect-error asynchronous adapter methods are unsupported
registry.register("x-invalid", {parse: async input => sdk.parse(input)});

const bundle = sdk.exportPreserved(document, "plain");
const restored: Document = sdk.restorePreserved(bundle.output, "plain", bundle.sidecar).document;
// @ts-expect-error a complete sidecar is required
sdk.restorePreserved(bundle.output, "plain", {});
const projected: Document = sdk.project(document, {profile:"custom", features:[]}).document;
sdk.exportProfile(projected, "json", {profile:"full"}, "strict");
// @ts-expect-error unsupported profile names must remain a compile-time error
sdk.project(document, {profile:"invented"});
const shifted: Document = sdk.normalize(document, -250000).document;
document.agents = [{id:"singer",kind:"person",attributes:{}}];
// @ts-expect-error unsupported formats must remain a compile-time error
sdk.parse("hello", "not-a-format");
''')
    run(["tsc", "--noEmit", "--strict", "--target", "ES2022", "--module", "NodeNext", "--moduleResolution", "NodeNext", types], cwd=javascript, env=env)

    cpp = work / "cpp"
    cpp.mkdir()
    write(cpp, "CMakeLists.txt", '''cmake_minimum_required(VERSION 3.20)
project(Consumer LANGUAGES C CXX)
find_package(UniLyrics 0.1 CONFIG REQUIRED)
add_executable(consumer_c main.c)
target_link_libraries(consumer_c PRIVATE UniLyrics::unilyrics)
add_executable(consumer_cpp main.cpp)
target_link_libraries(consumer_cpp PRIVATE UniLyrics::cpp)
add_executable(registry_c registry.c)
target_link_libraries(registry_c PRIVATE UniLyrics::unilyrics)
add_executable(registry_cpp registry.cpp)
target_link_libraries(registry_cpp PRIVATE UniLyrics::cpp)
''')
    write(cpp, "main.c", '''#include <unilyrics.h>
#include <assert.h>
#include <string.h>
int main(void) {
    const char *xml = "<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s'><span begin='250ms'>voice</span></p></body></tt>";
    char *ttml = unilyrics_parse((const uint8_t *)xml, strlen(xml), UNILYRICS_TTML);
    assert(ttml && strstr(ttml, "1250000"));
    unilyrics_free(ttml);
    const char *text = "[00:01.25]hello";
    char *reply = unilyrics_parse((const uint8_t *)text, strlen(text), UNILYRICS_LRC);
    assert(reply && strstr(reply, "1250000"));
    unilyrics_free(reply);
    const char *doc = "{\\"version\\":1,\\"cues\\":[],\\"metadata\\":[],\\"attributes\\":{}}";
    reply = unilyrics_validate((const uint8_t *)doc, strlen(doc));
    assert(reply && strstr(reply, "\\"ok\\":true"));
    unilyrics_free(reply);
    reply = unilyrics_normalize((const uint8_t *)doc, strlen(doc), 100);
    assert(reply && strstr(reply, "diagnostics"));
    unilyrics_free(reply);
    reply = unilyrics_project((const uint8_t *)doc, strlen(doc), NULL, 0);
    assert(reply && strstr(reply, "diagnostics"));
    unilyrics_free(reply);
    reply = unilyrics_export_profile((const uint8_t *)doc, strlen(doc), UNILYRICS_JSON, 1, NULL, 0);
    assert(reply && strstr(reply, "output"));
    unilyrics_free(reply);
    reply = unilyrics_export((const uint8_t *)doc, strlen(doc), UNILYRICS_PLAIN, 1);
    assert(reply && strstr(reply, "LOSSY_CONVERSION"));
    unilyrics_free(reply);
    reply = unilyrics_export_preserved((const uint8_t *)doc, strlen(doc), UNILYRICS_PLAIN, NULL, 0);
    assert(reply && strstr(reply, "sidecar"));
    unilyrics_free(reply);
    const char *sidecar = "{\\"version\\":1,\\"format\\":\\"plain\\",\\"output\\":\\"\\",\\"document\\":{\\"version\\":1,\\"cues\\":[]}}";
    reply = unilyrics_restore_preserved(NULL, 0, UNILYRICS_PLAIN, (const uint8_t *)sidecar, strlen(sidecar));
    assert(reply && strstr(reply, "SIDECAR_RESTORED"));
    unilyrics_free(reply);
    reply = unilyrics_inspect((const uint8_t *)doc, strlen(doc));
    assert(reply && strstr(reply, "auxiliary_kinds"));
    unilyrics_free(reply);
    return 0;
}
''')
    write(cpp, "main.cpp", '''#include <unilyrics.hpp>
#include <cassert>
int main() {
    assert(unilyrics::parse("<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s'><span begin='250ms'>voice</span></p></body></tt>", UNILYRICS_TTML).find("1250000") != std::string::npos);
    assert(unilyrics::parse("[00:01.25]你好").find("1250000") != std::string::npos);
    assert(unilyrics::detect("[00:01]hi").find("lrc") != std::string::npos);
    assert(unilyrics::project(R"({"version":1,"cues":[{"id":"cue","start_us":100,"segments":[{"text":"Hello"}]}]})", R"({"profile":"plain"})").find("PROFILE_LINE_TIMING_DROPPED") != std::string::npos);
    assert(unilyrics::normalize(R"({"version":1,"cues":[{"id":"cue","start_us":100,"segments":[]}]})", -100).find("TIMING_SHIFTED") != std::string::npos);
    const auto doc = R"({"version":1,"cues":[],"metadata":[],"attributes":{}})";
    assert(unilyrics::export_profile(doc, UNILYRICS_JSON).find("output") != std::string::npos);
    assert(unilyrics::export_lyrics(doc, UNILYRICS_PLAIN, true).find("LOSSY_CONVERSION") != std::string::npos);
    assert(unilyrics::export_preserved(doc, UNILYRICS_PLAIN).find("sidecar") != std::string::npos);
    assert(unilyrics::restore_preserved("", UNILYRICS_PLAIN, R"({"version":1,"format":"plain","output":"","document":{"version":1,"cues":[]}})").find("SIDECAR_RESTORED") != std::string::npos);
    assert(unilyrics::inspect(doc).find("auxiliary_kinds") != std::string::npos);
    assert(unilyrics::validate(doc).find("\\"ok\\":true") != std::string::npos);
    assert(unilyrics::export_lyrics(doc, UNILYRICS_PLAIN).find("\\"output\\":\\"\\"") != std::string::npos);
}
''')
    write(cpp, "registry.c", (ROOT / "bindings/c/registry_test.c").read_text())
    write(cpp, "registry.cpp", (ROOT / "bindings/cpp/registry_test.cpp").read_text())
    run(["cmake", "-S", cpp, "-B", cpp / "build", "-DCMAKE_PREFIX_PATH=" + str(sdk)], env=env)
    run(["cmake", "--build", cpp / "build"], env=env)
    run([cpp / "build/consumer_c"], cwd=cpp, env=env)
    run([cpp / "build/consumer_cpp"], cwd=cpp, env=env)
    run([cpp / "build/registry_c"], cwd=cpp, env=env)
    run([cpp / "build/registry_cpp"], cwd=cpp, env=env)

    java = work / "java"
    java.mkdir()
    system_native = work / "java-native"
    system_native.mkdir()
    with zipfile.ZipFile(packages / f"unilyrics-{version}-linux-x86_64.jar") as archive:
        for name in ("libunilyrics_ffi.so", "libunilyrics_jni.so"):
            (system_native / name).write_bytes(archive.read("org/tharukrj/unilyrics/native/linux-x86_64/" + name))
    classpath = os.pathsep.join([str(java), str(packages / f"unilyrics-{version}-linux-x86_64.jar"), str(packages / "json-20250517.jar")])
    source = write(java, "Consumer.java", '''import org.tharukrj.unilyrics.UniLyrics;
import org.tharukrj.unilyrics.LyricsModel;
import org.tharukrj.unilyrics.Registry;
import java.util.Map;
public class Consumer {
    public static void main(String[] args) {
        if (args.length > 0) { UniLyrics.loadSystem(); UniLyrics.loadSystem(); }
        else { UniLyrics.load(); UniLyrics.load(); }
        var ttml = UniLyrics.parse("<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s' end='3s'><span begin='250ms' end='1s'>voice</span></p></body></tt>", "ttml");
        @SuppressWarnings("unchecked")
        var ttmlDocument = (Map<String,Object>) ttml.get("document");
        String ttmlOutput = (String)UniLyrics.export(ttmlDocument, "ttml", "strict").get("output");
        if (!UniLyrics.parse(ttmlOutput, "ttml").get("document").equals(ttmlDocument)) throw new AssertionError("TTML round trip");
        if (!UniLyrics.detect("[00:01]hi").equals("lrc")) throw new AssertionError("detect");
        var result = UniLyrics.parse("[00:01.25]සිංහල 👩🏽‍💻", "lrc");
        @SuppressWarnings("unchecked")
        var document = (Map<String,Object>) result.get("document");
        @SuppressWarnings("unchecked")
        var attributes = (Map<String,Object>) document.get("attributes");
        attributes.put("nullable", null);
        UniLyrics.validate(document);
        if (!UniLyrics.inspect(document).get("line_timing").equals(true)) throw new AssertionError("inspect");
        String encoded = (String)UniLyrics.export(document, "json", "strict").get("output");
        if (!UniLyrics.parse(encoded, "json").get("document").equals(document)) throw new AssertionError("null preservation");
        if (!UniLyrics.export(document, "plain").get("output").equals("සිංහල 👩🏽‍💻")) throw new AssertionError("export");
        try { UniLyrics.parse("[00:60]bad", "lrc"); throw new AssertionError("missing error"); }
        catch (UniLyrics.LyricsException error) { if (!error.getCode().equals("INVALID_TIMESTAMP")) throw error; }
        var typed = UniLyrics.parseTyped("[00:01.25]සිංහල 👩🏽‍💻", LyricsModel.Format.LRC);
        if (typed.document().cues().get(0).startUs() != 1_250_000L) throw new AssertionError("typed timestamp");
        UniLyrics.validate(typed.document());
        if (!UniLyrics.inspect(typed.document()).lineTiming()) throw new AssertionError("typed features");
        var bundle = UniLyrics.exportPreserved(typed.document(), LyricsModel.Format.PLAIN);
        if (bundle.losses().isEmpty() || !UniLyrics.restorePreserved(bundle.output(), LyricsModel.Format.PLAIN, bundle.sidecar()).document().equals(typed.document())) throw new AssertionError("preservation");
        var large = UniLyrics.parseTyped("</".repeat(120_000), LyricsModel.Format.PLAIN).document();
        try { UniLyrics.exportPreserved(large, LyricsModel.Format.PLAIN); throw new AssertionError("oversized restore bundle returned"); }
        catch (UniLyrics.LyricsException error) { if (!error.getCode().equals("INPUT_TOO_LARGE")) throw error; }
        var blank = UniLyrics.parseTyped("", LyricsModel.Format.PLAIN).document();
        try { UniLyrics.export(blank, LyricsModel.Format.ENHANCED_LRC, LyricsModel.Policy.STRICT); throw new AssertionError("empty segment loss accepted"); }
        catch (UniLyrics.LyricsException error) { if (!error.getCode().equals("LOSSY_CONVERSION")) throw error; }
        var profileOptions = new LyricsModel.ProjectionOptions(LyricsModel.Profile.PLAIN);
        var profiled = UniLyrics.project(typed.document(), profileOptions);
        if (profiled.document().cues().get(0).startUs() != null) throw new AssertionError("profile projection");
        if (!UniLyrics.exportProfile(typed.document(), LyricsModel.Format.JSON, profileOptions).losses().equals(profiled.diagnostics())) throw new AssertionError("profile losses");

        if (UniLyrics.normalize(typed.document(), -250_000L).document().cues().get(0).startUs() != 1_000_000L) throw new AssertionError("normalization");
        var typedJSON = UniLyrics.export(typed.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT);
        if (!UniLyrics.parseTyped(typedJSON.output(), LyricsModel.Format.JSON).document().equals(typed.document())) throw new AssertionError("typed round trip");
        var authored = new LyricsModel.Document(java.util.List.of(new LyricsModel.Cue("cue", java.util.List.of(new LyricsModel.Segment("Hello")))));
        var plain = UniLyrics.export(authored, LyricsModel.Format.PLAIN);
        if (!plain.output().equals("Hello") || !plain.losses().get(0).code().equals("CUE_IDS_CHANGED")) throw new AssertionError("typed authoring");
        var registry = new Registry();
        registry.register("x-wrapped", new Registry.Adapter() {
            public boolean matches(String input) { return input.startsWith("CUSTOM:"); }
            public LyricsModel.ParseResult parse(String input) {
                if (!matches(input)) throw new IllegalArgumentException("Missing CUSTOM prefix");
                return UniLyrics.parseTyped(input.substring("CUSTOM:".length()), LyricsModel.Format.JSON);
            }
            public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) {
                var result = UniLyrics.export(document, LyricsModel.Format.JSON, policy);
                return new LyricsModel.ExportResult("CUSTOM:" + result.output(), result.losses());
            }
        });
        var custom = registry.export(typed.document(), "x-wrapped", LyricsModel.Policy.STRICT);
        if (!custom.losses().isEmpty() || !registry.parse(custom.output()).document().equals(typed.document())) throw new AssertionError("custom registry");


    }
}
''')
    run(["javac", "--release", "17", "-classpath", classpath, "-d", java, source], cwd=java, env=env)
    run(["java", "-classpath", classpath, "Consumer"], cwd=java, env=env)
    run(["java", f"-Djava.library.path={system_native}", "-classpath", classpath, "Consumer", "--system"], cwd=java, env=env)
    kotlin = write(java, "Consumer.kt", '''import org.tharukrj.unilyrics.Lyrics
import org.tharukrj.unilyrics.LyricsModel
import org.tharukrj.unilyrics.Registry
fun main(args: Array<String>) {
    if (args.isNotEmpty()) { Lyrics.loadSystem(); Lyrics.loadSystem() }
    else Lyrics.load()
    @Suppress("UNCHECKED_CAST")
    val ttml = Lyrics.parse("<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s' end='3s'><span begin='250ms' end='1s'>voice</span></p></body></tt>", "ttml")["document"] as Map<String,Any?>
    val encoded = Lyrics.export(ttml, "ttml", "strict")["output"] as String
    check(Lyrics.parse(encoded, "ttml")["document"] == ttml)
    check(Lyrics.detect("[00:01]hi") == "lrc")
    @Suppress("UNCHECKED_CAST")
    val document = Lyrics.parse("සිංහල 👩🏽‍💻", "plain")["document"] as Map<String,Any?>
    Lyrics.validate(document)
    check(Lyrics.inspect(document)["line_timing"] == false)
    check(Lyrics.export(document, "plain")["output"] == "සිංහල 👩🏽‍💻")
    val typed = Lyrics.parseTyped("[00:01.25]සිංහල 👩🏽‍💻", LyricsModel.Format.LRC)
    check(typed.document().cues()[0].startUs() == 1_250_000L)
    Lyrics.validate(typed.document())
    check(Lyrics.inspect(typed.document()).lineTiming())
    val bundle = Lyrics.exportPreserved(typed.document(), LyricsModel.Format.PLAIN)
    check(bundle.losses().isNotEmpty())
    check(Lyrics.restorePreserved(bundle.output(), LyricsModel.Format.PLAIN, bundle.sidecar()).document() == typed.document())
    val large = Lyrics.parseTyped("</".repeat(120_000), LyricsModel.Format.PLAIN).document()
    try { Lyrics.exportPreserved(large, LyricsModel.Format.PLAIN); error("oversized restore bundle returned") }
    catch (expected: org.tharukrj.unilyrics.UniLyrics.LyricsException) { check(expected.code == "INPUT_TOO_LARGE") }
    val blank = Lyrics.parseTyped("", LyricsModel.Format.PLAIN).document()
    try { Lyrics.export(blank, LyricsModel.Format.ENHANCED_LRC, LyricsModel.Policy.STRICT); error("empty segment loss accepted") }
    catch (expected: org.tharukrj.unilyrics.UniLyrics.LyricsException) { check(expected.code == "LOSSY_CONVERSION") }
    val profileOptions = LyricsModel.ProjectionOptions(LyricsModel.Profile.PLAIN)
    val profiled = Lyrics.project(typed.document(), profileOptions)
    check(profiled.document().cues()[0].startUs() == null)
    check(Lyrics.exportProfile(typed.document(), LyricsModel.Format.JSON, profileOptions).losses() == profiled.diagnostics())

    check(Lyrics.normalize(typed.document(), -250_000L).document().cues()[0].startUs() == 1_000_000L)
    val typedJSON = Lyrics.export(typed.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT)
    check(Lyrics.parseTyped(typedJSON.output(), LyricsModel.Format.JSON).document() == typed.document())
    val authored = LyricsModel.Document(listOf(LyricsModel.Cue("cue", listOf(LyricsModel.Segment("Hello")))))
    val plain = Lyrics.export(authored, LyricsModel.Format.PLAIN)
    check(plain.output() == "Hello")
    check(plain.losses().map { it.code() } == listOf("CUE_IDS_CHANGED"))
    val registry = Registry()
    registry.register("x-wrapped", object : Registry.Adapter {
        override fun matches(input: String) = input.startsWith("CUSTOM:")
        override fun parse(input: String): LyricsModel.ParseResult {
            check(matches(input))
            return Lyrics.parseTyped(input.removePrefix("CUSTOM:"), LyricsModel.Format.JSON)
        }
        override fun export(document: LyricsModel.Document, policy: LyricsModel.Policy): LyricsModel.ExportResult {
            val result = Lyrics.export(document, LyricsModel.Format.JSON, policy)
            return LyricsModel.ExportResult("CUSTOM:" + result.output(), result.losses())
        }
    })
    val custom = registry.export(typed.document(), "x-wrapped", LyricsModel.Policy.STRICT)
    check(custom.losses().isEmpty())
    check(registry.parse(custom.output()).document() == typed.document())


}
''')
    kotlin_classpath = classpath + os.pathsep + str(packages / f"unilyrics-kotlin-{version}.jar")
    run(["kotlinc", kotlin, "-jvm-target", "17", "-classpath", kotlin_classpath, "-include-runtime", "-d", java / "consumer.jar"], cwd=java, env=env)
    run(["java", "-classpath", str(java / "consumer.jar") + os.pathsep + kotlin_classpath, "ConsumerKt"], cwd=java, env=env)
    run(["java", f"-Djava.library.path={system_native}", "-classpath", str(java / "consumer.jar") + os.pathsep + kotlin_classpath, "ConsumerKt", "--system"], cwd=java, env=env)

    go = work / "go"
    go.mkdir()
    write(go, "go.mod", f'''module consumer.local/test
go 1.22
require github.com/TharukRenuja/UniLyrics/bindings/go v0.0.0
replace github.com/TharukRenuja/UniLyrics/bindings/go => {sdk / 'bindings/go'}
''')
    write(go, "main.go", '''package main
import ("fmt"; "strings"; lyrics "github.com/TharukRenuja/UniLyrics/bindings/go")
func main() {
    ttml, err := lyrics.Parse("<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s' end='3s'><span begin='250ms' end='1s'>voice</span></p></body></tt>", "ttml")
    if err != nil || *ttml.Document.Cues[0].Segments[0].Start != 1250000 { panic(fmt.Sprint(ttml, err)) }
    encoded, err := lyrics.Export(ttml.Document, "ttml", "strict")
    if err != nil || encoded.Output == "" { panic(fmt.Sprint(encoded, err)) }
    format, err := lyrics.Detect("[00:01]hi")
    if err != nil || format != "lrc" { panic(fmt.Sprint(format, err)) }
    result, err := lyrics.Parse("[00:01.25]සිංහල 👩🏽‍💻", "lrc")
    if err != nil { panic(err) }
    if err := lyrics.Validate(result.Document); err != nil { panic(err) }
    bundle, err := lyrics.ExportPreserved(result.Document, "plain", lyrics.ProjectionOptions{})
    if err != nil || len(bundle.Losses) == 0 { panic("preservation export") }
    restored, err := lyrics.RestorePreserved(bundle.Output, "plain", bundle.Sidecar)
    if err != nil || *restored.Document.Cues[0].Start != 1250000 { panic("preservation restore") }
    large, err := lyrics.Parse(strings.Repeat("<", 100000), "plain")
    if err != nil { panic(err) }
    _, err = lyrics.ExportPreserved(large.Document, "plain", lyrics.ProjectionOptions{})
    sizeError, ok := err.(*lyrics.Error)
    if !ok || sizeError.Code != "INPUT_TOO_LARGE" { panic("oversized restore bundle returned") }
    blank, err := lyrics.Parse("", "plain")
    if err != nil { panic(err) }
    _, err = lyrics.Export(blank.Document, "enhanced_lrc", "strict")
    lossError, ok := err.(*lyrics.Error)
    if !ok || lossError.Code != "LOSSY_CONVERSION" { panic("empty segment loss accepted") }
    profileOptions := lyrics.ProjectionOptions{Profile:lyrics.ProfilePlain}
    profiled, err := lyrics.Project(result.Document, profileOptions)
    if err != nil { panic(err) }
    if profiled.Document.Cues[0].Start != nil { panic("projection kept timing") }
    profileExport, err := lyrics.ExportProfile(result.Document, "json", profileOptions, "")
    if err != nil || len(profileExport.Losses) != len(profiled.Diagnostics) { panic("profile export") }

    shifted, err := lyrics.Normalize(result.Document, -250000)
    if err != nil || *shifted.Document.Cues[0].Start != 1000000 || *result.Document.Cues[0].Start != 1250000 { panic(fmt.Sprint(shifted,err)) }
    features, err := lyrics.Inspect(result.Document)
    if err != nil || !features.LineTiming { panic(fmt.Sprint(features,err)) }
    output, err := lyrics.Export(result.Document, "plain", "compatible")
    if err != nil || output.Output != "සිංහල 👩🏽‍💻" { panic(fmt.Sprint(output,err)) }
    var registry lyrics.Registry
    err = registry.Register("x-custom", lyrics.Adapter{
        Matches: func(input string) bool { return strings.HasPrefix(input, "CUSTOM:") },
        Parse: func(input string) (lyrics.ParseResult, error) {
            input, ok := strings.CutPrefix(input, "CUSTOM:")
            if !ok { return lyrics.ParseResult{}, fmt.Errorf("missing CUSTOM prefix") }
            return lyrics.Parse(input, "json")
        },
        Export: func(document lyrics.Document, policy string) (lyrics.ExportResult, error) {
            result, err := lyrics.Export(document, "json", policy)
            result.Output = "CUSTOM:" + result.Output
            return result, err
        },
    })
    if err != nil { panic(err) }
    custom, err := registry.Export(result.Document, "x-custom", "strict")
    if err != nil || len(custom.Losses) != 0 { panic(fmt.Sprint(custom,err)) }
    parsedCustom, err := registry.Parse(custom.Output, "")
    if err != nil || *parsedCustom.Document.Cues[0].Start != 1250000 { panic(fmt.Sprint(parsedCustom,err)) }

}
''')
    go_env = env.copy()
    go_env.update({"CGO_ENABLED": "1", "CGO_LDFLAGS": "-L" + str(sdk / "lib") + " -Wl,-rpath," + str(sdk / "lib"),
                   "GOCACHE": str(ROOT / "target/go-cache"), "GOTOOLCHAIN": "local", "GOPROXY": "off"})
    run(["go", "run", "."], cwd=go, env=go_env)

    swift = work / "swift-consumer"
    swift.mkdir()
    write(swift, "Package.swift", f'''// swift-tools-version: 5.9
import PackageDescription
let package = Package(name:"Consumer", dependencies:[.package(path:"{sdk / 'bindings/swift'}")],
    targets:[.executableTarget(name:"Consumer", dependencies:[.product(name:"UniLyrics",package:"swift")])])
''')
    write(swift, "Sources/Consumer/main.swift", '''import Foundation
import UniLyrics
let ttml = try UniLyrics.parse("<tt xmlns='http://www.w3.org/ns/ttml'><body><p begin='1s' end='3s'><span begin='250ms' end='1s'>voice</span></p></body></tt>", format:"ttml") as! [String:Any]
let ttmlDocument = ttml["document"] as! [String:Any]
let ttmlEncoded = try UniLyrics.export(ttmlDocument, format:"ttml", policy:"strict") as! [String:Any]
let ttmlReparsed = try UniLyrics.parse(ttmlEncoded["output"] as! String, format:"ttml") as! [String:Any]
let ttmlBefore = try JSONSerialization.data(withJSONObject: ttmlDocument, options:.sortedKeys)
let ttmlAfter = try JSONSerialization.data(withJSONObject: ttmlReparsed["document"]!, options:.sortedKeys)
precondition(ttmlBefore == ttmlAfter)
let format = try UniLyrics.detect("[00:01]hi")
precondition(format == "lrc")
let parsed = try UniLyrics.parse("සිංහල 👩🏽‍💻", format:"plain") as! [String:Any]
let document = parsed["document"] as! [String:Any]
try UniLyrics.validate(document)
let features = try UniLyrics.inspect(document)
precondition(features["line_timing"] as? Bool == false)
let output = try UniLyrics.export(document, format:"plain") as! [String:Any]
precondition(output["output"] as? String == "සිංහල 👩🏽‍💻")
let typed = try UniLyrics.parseTyped("[00:01.25]සිංහල 👩🏽‍💻", format: .lrc)
precondition(typed.document.cues[0].startUs == 1_250_000)
try UniLyrics.validate(typed.document)
let typedFeatures = try UniLyrics.inspect(typed.document)
precondition(typedFeatures.lineTiming)
let bundle = try UniLyrics.exportPreserved(typed.document, format: .plain)
precondition(!bundle.losses.isEmpty)
let restored = try UniLyrics.restorePreserved(bundle.output, format: .plain, sidecar: bundle.sidecar)
precondition(restored.document == typed.document)
let large = try UniLyrics.parseTyped(String(repeating: "/", count: 200_000), format: .plain).document
do { _ = try UniLyrics.exportPreserved(large, format: .plain); preconditionFailure("oversized restore bundle returned") }
catch let error as UniLyricsError { precondition(error.code == "INPUT_TOO_LARGE") }
let blank = try UniLyrics.parseTyped("", format: .plain).document
do { _ = try UniLyrics.export(blank, format: .enhancedLRC, policy: .strict); preconditionFailure("empty segment loss accepted") }
catch let error as UniLyricsError { precondition(error.code == "LOSSY_CONVERSION") }
let profileOptions = ProjectionOptions(profile: .plain)
let profiled = try UniLyrics.project(typed.document, options: profileOptions)
precondition(profiled.document.cues[0].startUs == nil)
let profileExport = try UniLyrics.exportProfile(typed.document, format: .json, options: profileOptions)
precondition(profileExport.losses == profiled.diagnostics)

let shifted = try UniLyrics.normalize(typed.document, offsetUs: -250_000)
precondition(shifted.document.cues[0].startUs == 1_000_000)
precondition(typed.document.cues[0].startUs == 1_250_000)
let typedJSON = try UniLyrics.export(typed.document, format: .json, policy: .strict)
let typedAgain = try UniLyrics.parseTyped(typedJSON.output, format: .json)
precondition(typedAgain.document == typed.document)
let authored = LyricsDocument(cues: [LyricsCue(id: "cue", segments: [LyricsSegment(text: "Hello")])])
let authoredPlain = try UniLyrics.export(authored, format: .plain)
precondition(authoredPlain.output == "Hello")
precondition(authoredPlain.losses.map { $0.code } == ["CUE_IDS_CHANGED"])
let registry = LyricsRegistry()
try registry.register("x-wrapped", adapter: LyricsAdapter(
    matches: { $0.hasPrefix("CUSTOM:") },
    parse: { input in
        guard input.hasPrefix("CUSTOM:") else { throw UniLyricsError(code: "BAD_HEADER", message: "Missing CUSTOM prefix") }
        return try UniLyrics.parseTyped(String(input.dropFirst("CUSTOM:".count)), format: .json)
    },
    export: { document, policy in
        var result = try UniLyrics.export(document, format: .json, policy: policy)
        result.output = "CUSTOM:" + result.output
        return result
    }))
let custom = try registry.export(typed.document, format: "x-wrapped", policy: .strict)
precondition(custom.losses.isEmpty)
let customFormat = try registry.detect(custom.output)
precondition(customFormat == "x-wrapped")
let customParsed = try registry.parse(custom.output)
precondition(customParsed.document == typed.document)


''')
    swift_env = env.copy()
    swift_env["CLANG_MODULE_CACHE_PATH"] = str(ROOT / "target/swift-clang-cache")
    swift_env["SWIFTPM_MODULECACHE_OVERRIDE"] = str(ROOT / "target/swift-module-cache")
    run(["swift", "run", "--cache-path", ROOT / "target/swift-cache", "--config-path", ROOT / "target/swift-config",
         "--security-path", ROOT / "target/swift-security", "-Xlinker", "-L" + str(sdk / "lib"),
         "-Xlinker", "-rpath", "-Xlinker", sdk / "lib", "Consumer"], cwd=swift, env=swift_env)

    rust = work / "rust"
    rust.mkdir()
    with tarfile.open(packages / f"unilyrics-{version}.crate") as archive:
        archive.extractall(rust, filter="data")
    run(["cargo", "test", "--offline", "--manifest-path", rust / f"unilyrics-{version}/Cargo.toml"], cwd=rust, env=env)
    write(rust, "Cargo.toml", f'''[package]
name = "consumer"
version = "0.0.0"
edition = "2021"
[dependencies]
unilyrics = {{path = "unilyrics-{version}"}}
''')
    write(rust, "src/main.rs", '''use unilyrics::{parse, export, Format, Policy};
fn main() {
    let result = parse("[00:01.25]සිංහල 👩🏽‍💻", Some(Format::Lrc)).unwrap();
    assert_eq!(result.document.cues[0].start_us, Some(1_250_000));
    assert!(unilyrics::inspect(&result.document).unwrap().line_timing);
    let options = unilyrics::ProjectionOptions { profile: unilyrics::Profile::Plain, features: None };
    let bundle = unilyrics::export_preserved(&result.document, Format::Plain, &options).unwrap();
    assert!(!bundle.losses.is_empty());
    assert_eq!(unilyrics::restore_preserved(&bundle.output, Format::Plain, &bundle.sidecar).unwrap().document, result.document);
    let projected = unilyrics::project(&result.document, &options).unwrap();
    assert_eq!(projected.document.cues[0].start_us, None);
    assert!(unilyrics::export_profile(&result.document, Format::Json, &options, Policy::Strict).is_err());

    assert_eq!(unilyrics::normalize(&result.document, -250_000).unwrap().document.cues[0].start_us, Some(1_000_000));
    assert_eq!(export(&result.document, Format::Plain, Policy::Compatible).unwrap().output, "සිංහල 👩🏽‍💻");
    struct CustomText;
    impl unilyrics::CustomFormat for CustomText {
        fn matches(&self, text: &str) -> bool { text.starts_with("CUSTOM:") }
        fn parse(&self, text: &str) -> Result<unilyrics::ParseResult, unilyrics::Error> {
            let text = text.strip_prefix("CUSTOM:").ok_or_else(|| unilyrics::Error::new("BAD_HEADER", "Missing CUSTOM prefix"))?;
            parse(text, Some(Format::Plain))
        }
        fn export(&self, document: &unilyrics::Document, policy: Policy) -> Result<unilyrics::ExportResult, unilyrics::Error> {
            let mut result = export(document, Format::Plain, policy)?;
            result.output = format!("CUSTOM:{}", result.output);
            Ok(result)
        }
    }
    let mut registry = unilyrics::Registry::default();
    registry.register("x-custom", CustomText).unwrap();
    let custom = registry.parse("CUSTOM:hello", None).unwrap();
    let encoded = registry.export(&custom.document, "x-custom", Policy::Strict).unwrap();
    assert_eq!(encoded.output, "CUSTOM:hello");
    assert!(encoded.losses.is_empty());

}
''')
    run(["cargo", "run", "--offline"], cwd=rust, env=env)
    print("Installed-package consumers passed for all 10 original languages.")


if __name__ == "__main__":
    main()
