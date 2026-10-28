import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { initialize, UniLyricsError, Registry } from "./index.js";
const source = await readFile(new URL("./dist/core.wasm", import.meta.url));
const sdk = await initialize(source);
assert.throws(() => sdk.process({ op: "parse", input: NaN }), TypeError);
assert.throws(() => sdk.process({ op: "parse", input: undefined }), TypeError);
const fixtures = JSON.parse(await readFile(new URL("../../crates/unilyrics/tests/fixtures/conformance.json", import.meta.url), "utf8"));
for (const test of fixtures) {
  const input = test.input ?? JSON.stringify(test.document);
  if (test.error) {
    assert.throws(() => sdk.parse(input, test.format), error => error instanceof UniLyricsError && error.code === test.error);
  } else {
    const { document } = sdk.parse(input, test.format);
    for (const expected of test.exports ?? []) {
      const encoded = sdk.export(document, expected.format, expected.policy);
      assert.equal(encoded.output, expected.output, test.name);
      assert.deepEqual(encoded.losses, []);
    }
    assert.deepEqual(document.cues.map(cue => cue.segments.map(s => s.text).join("")), test.text, test.name);
    assert.deepEqual(document.cues.map(cue => cue.start_us), test.starts, test.name);
    assert.deepEqual(sdk.parse(sdk.export(document, "json", "strict").output, "json").document, document);
    if (test.features) {
      assert.deepEqual(document, test.document);
      assert.deepEqual(sdk.inspect(document), test.features);
      sdk.validate(document);
      assert.deepEqual([...new Set(sdk.export(document, "plain").losses.map(loss => loss.code))].sort(), [...test.loss_codes].sort());
      assert.throws(() => sdk.export(document, "plain", "strict"), {code:"LOSSY_CONVERSION"});
    }
  }
}
const module = await WebAssembly.compile(source);
assert.equal((await initialize(module)).detect("[00:01]hello"), "lrc");
for (let i = 0; i < 1000; i++) assert.equal(sdk.parse("你好", "plain").document.cues[0].segments[0].text, "你好");
console.log("JS/WASM conformance, module loading and repeated-call checks passed.");

const normalizationCases = JSON.parse(await readFile(new URL("../../crates/unilyrics/tests/fixtures/normalization.json", import.meta.url), "utf8"));
for (const test of normalizationCases) {
  const before = JSON.stringify(test.document);
  if (test.error) {
    assert.throws(() => sdk.normalize(test.document, test.offset_us), error => error.code === test.error, test.name);
  } else {
    const result = sdk.normalize(test.document, test.offset_us);
    assert.deepEqual(result.document, test.expected_document, test.name);
    assert.deepEqual(result.diagnostics.map(d => d.code), test.diagnostic_codes, test.name);
  }
  assert.equal(JSON.stringify(test.document), before, test.name);
}


const registry = new Registry(sdk);
const wrapped = {
  prefix: "CUSTOM\n",
  matches(input) { return input.startsWith(this.prefix); },
  parse(input) {
    if (!input.startsWith(this.prefix)) throw new UniLyricsError("BAD_HEADER", "Missing CUSTOM header");
    return sdk.parse(input.slice(this.prefix.length), "json");
  },
  export(document, policy) {
    const result = sdk.export(document, "json", policy);
    return {...result, output: this.prefix + result.output};
  },
};
registry.register("x-wrapped", wrapped);
for (const test of fixtures.filter(test => test.document && !test.error)) {
  const before = JSON.stringify(test.document);
  const encoded = registry.export(test.document, "x-wrapped", "strict");
  assert.deepEqual(encoded.losses, []);
  assert.equal(registry.detect(encoded.output), "x-wrapped");
  assert.deepEqual(registry.parse(encoded.output).document, test.document, test.name);
  assert.equal(JSON.stringify(test.document), before);
}
for (const name of ["plain", "x-", "X-custom", "x-💻", "x-ok\n", "x-" + "a".repeat(63)]) {
  assert.throws(() => registry.register(name, {}), {code: "INVALID_CUSTOM_FORMAT"});
}
assert.throws(() => registry.register("x-wrapped", {}), {code: "DUPLICATE_FORMAT"});
assert.throws(() => new Registry(sdk).parse("CUSTOM\n{}", "x-wrapped"), {code: "UNKNOWN_FORMAT"});
assert.deepEqual(registry.parse("hello", "plain"), sdk.parse("hello", "plain"));
registry.register("x-second", wrapped);
assert.throws(() => registry.detect("CUSTOM\n{}"), {code: "AMBIGUOUS_FORMAT"});
const broad = new Registry(sdk);
broad.register("x-broad", {matches: () => true});
assert.throws(() => broad.detect("[00:01]hi"), {code: "AMBIGUOUS_FORMAT"});
assert.equal(broad.parse("[00:01]hi", "lrc").document.cues[0].start_us, 1000000);
const checks = new Registry(sdk);
checks.register("x-bad", {parse: () => ({document: {version: 999, cues: []}, diagnostics: []})});
assert.throws(() => checks.parse("bad", "x-bad"), {code: "UNSUPPORTED_VERSION"});
assert.throws(() => checks.parse("x".repeat(1048577), "x-bad"), {code: "INPUT_TOO_LARGE"});
assert.throws(() => checks.parse("\uD800", "x-bad"), {code: "INVALID_UTF8"});
const original = sdk.parse("Hello", "plain").document;
const before = JSON.stringify(original);
checks.register("x-mutate", {
  parse: input => wrapped.parse(input),
  export(document, policy) {
    document.cues[0].segments[0].text = "changed";
    return wrapped.export(document, policy);
  },
});
assert.deepEqual(checks.export(original, "x-mutate").losses.map(d => d.code), ["CUSTOM_ROUNDTRIP_CHANGED"]);
assert.throws(() => checks.export(original, "x-mutate", "strict"), {code: "LOSSY_CONVERSION"});
assert.equal(JSON.stringify(original), before);
checks.register("x-output", {export: () => ({output: "opaque", losses: []})});
assert.deepEqual(checks.export(original, "x-output").losses.map(d => d.code), ["CUSTOM_ROUNDTRIP_UNVERIFIED"]);
assert.throws(() => checks.export(original, "x-output", "strict"), {code: "LOSSY_CONVERSION"});
assert.throws(() => checks.parse("opaque", "x-output"), {code: "UNSUPPORTED_OPERATION"});
let reparsedLarge = false;
checks.register("x-large", {export: () => ({output: "x".repeat(1048577), losses: []}), parse() { reparsedLarge = true; return sdk.parse("", "plain"); }});
assert.throws(() => checks.export(original, "x-large"), {code: "INPUT_TOO_LARGE"});
assert.equal(reparsedLarge, false);
checks.register("x-reported", {parse: input => wrapped.parse(input), export(document, policy) {
  const result = wrapped.export(document, policy);
  result.losses.push({code: "CUSTOM_LOSS", message: "reported", cue_id: null});
  return result;
}});
assert.throws(() => checks.export(original, "x-reported", "strict"), {code: "LOSSY_CONVERSION"});
console.log("JS custom registry validation, rich round trips and preservation checks passed.");

assert.throws(() => registry.parse("hi", "not-registered"), {code: "UNKNOWN_FORMAT"});
for (const test of fixtures.filter(test => !test.error)) {
  const input = Object.hasOwn(test, "input") ? test.input : JSON.stringify(test.document);
  assert.deepEqual(registry.parse(input, test.format), sdk.parse(input, test.format), test.name);
}


for (const test of JSON.parse(await readFile(new URL("../../crates/unilyrics/tests/fixtures/profiles.json", import.meta.url), "utf8"))) {
  const before = JSON.stringify(test.request);
  const request = test.request;
  const invoke = () => request.op === "project"
    ? sdk.project(request.document, request.options)
    : sdk.exportProfile(request.document, request.format, request.options, request.policy);
  if (test.error) assert.throws(invoke, {code: test.error});
  else assert.deepEqual(invoke(), sdk.process(request), test.name);
  if (test.error) assert.throws(() => sdk.process(test.request), {code: test.error});
  else {
    const result = sdk.process(test.request);
    if (test.expected_document) assert.deepEqual(result.document, test.expected_document, test.name);
    else assert.deepEqual(JSON.parse(result.output), test.output_document, test.name);
    assert.deepEqual((result.diagnostics ?? result.losses).map(d => d.code), test.codes, test.name);
  }
  assert.equal(JSON.stringify(test.request), before);
}

assert.deepEqual(sdk.project(original), sdk.process({op: "project", document: original}));

for (const test of JSON.parse(await readFile(new URL("../../crates/unilyrics/tests/fixtures/profiles.json", import.meta.url), "utf8"))) {
  if (test.error || test.request.op !== "project") continue;
  const {document, options} = test.request;
  const before = JSON.stringify(document);
  const preserved = sdk.exportPreserved(document, "json", options);
  assert.deepEqual(JSON.parse(preserved.output), test.expected_document, test.name);
  assert.deepEqual(preserved.losses.map(d => d.code), test.codes, test.name);
  assert.deepEqual(preserved.sidecar.document, document);
  assert.deepEqual(sdk.restorePreserved(preserved.output, "json", preserved.sidecar).document, document);
  assert.throws(() => sdk.restorePreserved(preserved.output + "\n", "json", preserved.sidecar), {code: "SIDECAR_MISMATCH"});
  assert.throws(() => sdk.restorePreserved(preserved.output, "plain", preserved.sidecar), {code: "SIDECAR_MISMATCH"});
  assert.throws(() => sdk.restorePreserved(preserved.output, "json", {...preserved.sidecar, version: 2}), {code: "UNSUPPORTED_SIDECAR_VERSION"});
  assert.equal(JSON.stringify(document), before);
}
const preservationDocument = sdk.parse("[00:01]සිංහල 👩🏽‍💻", "lrc").document;
const bundle = sdk.exportPreserved(preservationDocument, "plain");
assert(bundle.losses.length > 0);
assert.deepEqual(sdk.restorePreserved(bundle.output, "plain", bundle.sidecar).document, preservationDocument);
console.log("JS preservation helper round trips, profiles and stale-sidecar checks passed.");

const sizeDocument = sdk.parse("x", "plain").document;
const sizeSeed = sdk.exportPreserved(sizeDocument, "plain");
const seedBytes = new TextEncoder().encode(JSON.stringify({op:"restore_preserved", input:sizeSeed.output, format:"plain", sidecar:sizeSeed.sidecar})).length;
const sizeCount = 1 + Math.floor((1048576 - seedBytes) / 3);
sizeDocument.cues[0].segments[0].text = "x".repeat(sizeCount);
const sizeBundle = sdk.exportPreserved(sizeDocument, "plain");
assert.deepEqual(sdk.restorePreserved(sizeBundle.output, "plain", sizeBundle.sidecar).document, sizeDocument);
sizeDocument.cues[0].segments[0].text += "x";
assert.throws(() => sdk.exportPreserved(sizeDocument, "plain"), {code:"INPUT_TOO_LARGE"});
