import json
import os
from pathlib import Path
import sys
from typing import get_type_hints
from unilyrics import (
    UniLyrics, UniLyricsError, Registry, Agent, Auxiliary, Cue, Diagnostic, Document,
    ExportResult, Features, Metadata, ParseResult, Segment, ValidationResult,
    PreservationSidecar, PreservedExportResult,
)

# Resolve recursive JSON aliases and ensure optional rich fields stay optional.
for model in (Agent, Auxiliary, Cue, Diagnostic, Document, ExportResult, Features,
              Metadata, ParseResult, Segment, ValidationResult, PreservationSidecar, PreservedExportResult):
    assert set(get_type_hints(model)) == model.__required_keys__ | model.__optional_keys__
assert Document.__required_keys__ == {"version", "cues", "metadata", "attributes"}
assert Segment.__optional_keys__ == {"id", "language", "agent_ids", "roles"}
assert get_type_hints(UniLyrics.parse)["return"] is ParseResult
assert get_type_hints(UniLyrics.export)["return"] is ExportResult
assert get_type_hints(UniLyrics.inspect)["return"] is Features
assert get_type_hints(UniLyrics.validate)["return"] is ValidationResult
assert get_type_hints(UniLyrics.export_preserved)["return"] is PreservedExportResult
assert get_type_hints(UniLyrics.restore_preserved)["return"] is ParseResult

root = Path(__file__).resolve().parents[2]
if len(sys.argv) > 1:
    sdk = UniLyrics(sys.argv[1])
else:
    os.environ.pop("UNILYRICS_LIBRARY", None)
    sdk = UniLyrics()
    assert Path(sdk._library._name).parent.name == "_native", "Expected the installed bundled engine"
for case in json.loads((root / "crates/unilyrics/tests/fixtures/conformance.json").read_text(encoding="utf-8")):
    text = case["input"] if "input" in case else json.dumps(case["document"], ensure_ascii=False)
    if "error" in case:
        try:
            sdk.parse(text, case["format"])
        except UniLyricsError as error:
            assert error.code == case["error"], case["name"]
        else:
            raise AssertionError(case["name"])
    else:
        document: Document = sdk.parse(text, case["format"])["document"]
        for expected in case.get("exports", []):
            encoded = sdk.export(document, expected["format"], expected["policy"])
            assert encoded["output"] == expected["output"], case["name"]
            assert encoded["losses"] == []
        assert ["".join(s["text"] for s in c["segments"]) for c in document["cues"]] == case["text"]
        assert [c["start_us"] for c in document["cues"]] == case["starts"]
        assert sdk.parse(sdk.export(document, "json", "strict")["output"], "json")["document"] == document
        if "features" in case:
            assert document == case["document"]
            assert sdk.inspect(document) == case["features"]
            sdk.validate(document)
            assert {loss["code"] for loss in sdk.export(document, "plain")["losses"]} == set(case["loss_codes"])
            try:
                sdk.export(document, "plain", "strict")
                raise AssertionError("rich conversion did not report losses")
            except UniLyricsError as error:
                assert error.code == "LOSSY_CONVERSION"
for _ in range(1000):
    assert sdk.parse("සිංහල", "plain")["document"]["cues"][0]["segments"][0]["text"] == "සිංහල"
print("Python/native conformance and repeated-call checks passed.")

for case in json.loads((root / "crates/unilyrics/tests/fixtures/normalization.json").read_text(encoding="utf-8")):
    before = json.dumps(case["document"], ensure_ascii=False)
    if "error" in case:
        try:
            sdk.normalize(case["document"], case.get("offset_us", 0))
        except UniLyricsError as error:
            assert error.code == case["error"], case["name"]
        else:
            raise AssertionError(case["name"])
    else:
        result = sdk.normalize(case["document"], case.get("offset_us", 0))
        assert result["document"] == case["expected_document"], case["name"]
        assert [d["code"] for d in result["diagnostics"]] == case["diagnostic_codes"]
    assert json.dumps(case["document"], ensure_ascii=False) == before


registry = Registry(sdk)
def custom_parse(text):
    if not text.startswith("CUSTOM\n"):
        raise UniLyricsError("BAD_HEADER", "Missing CUSTOM header")
    return sdk.parse(text[len("CUSTOM\n"):], "json")
def custom_export(document, policy):
    result = sdk.export(document, "json", policy)
    result["output"] = "CUSTOM\n" + result["output"]
    return result
registry.register("x-wrapped", parser=custom_parse, exporter=custom_export, detector=lambda text: text.startswith("CUSTOM\n"))
for case in json.loads((root / "crates/unilyrics/tests/fixtures/conformance.json").read_text(encoding="utf-8")):
    if "document" not in case or "error" in case:
        continue
    before = json.dumps(case["document"], ensure_ascii=False)
    encoded = registry.export(case["document"], "x-wrapped", "strict")
    assert encoded["losses"] == []
    assert registry.detect(encoded["output"]) == "x-wrapped"
    assert registry.parse(encoded["output"])["document"] == case["document"]
    assert json.dumps(case["document"], ensure_ascii=False) == before

def expect_error(code, action):
    try:
        action()
    except UniLyricsError as error:
        assert error.code == code, error
    else:
        raise AssertionError("Expected " + code)

for name in ("plain", "x-", "X-custom", "x-💻", "x-ok\n", "x-" + "a" * 63):
    expect_error("INVALID_CUSTOM_FORMAT", lambda: registry.register(name))
expect_error("DUPLICATE_FORMAT", lambda: registry.register("x-wrapped"))
expect_error("UNKNOWN_FORMAT", lambda: Registry(sdk).parse("CUSTOM\n{}", "x-wrapped"))
assert registry.parse("hello", "plain") == sdk.parse("hello", "plain")
registry.register("x-second", detector=lambda _: True)
expect_error("AMBIGUOUS_FORMAT", lambda: registry.detect("CUSTOM\n{}"))
expect_error("AMBIGUOUS_FORMAT", lambda: registry.detect("[00:01]hi"))
assert registry.parse("[00:01]hi", "lrc")["document"]["cues"][0]["start_us"] == 1000000
checks = Registry(sdk)
checks.register("x-bad", parser=lambda _: {"document": {"version": 999, "cues": []}, "diagnostics": []})
expect_error("UNSUPPORTED_VERSION", lambda: checks.parse("bad", "x-bad"))
expect_error("INPUT_TOO_LARGE", lambda: checks.parse("x" * 1048577, "x-bad"))
original = sdk.parse("Hello", "plain")["document"]
before = json.dumps(original, ensure_ascii=False)
def mutate(document, policy):
    document["cues"][0]["segments"][0]["text"] = "changed"
    return custom_export(document, policy)
checks.register("x-mutate", parser=custom_parse, exporter=mutate)
assert [d["code"] for d in checks.export(original, "x-mutate")["losses"]] == ["CUSTOM_ROUNDTRIP_CHANGED"]
expect_error("LOSSY_CONVERSION", lambda: checks.export(original, "x-mutate", "strict"))
assert json.dumps(original, ensure_ascii=False) == before
checks.register("x-output", exporter=lambda document, policy: {"output": "opaque", "losses": []})
assert [d["code"] for d in checks.export(original, "x-output")["losses"]] == ["CUSTOM_ROUNDTRIP_UNVERIFIED"]
expect_error("LOSSY_CONVERSION", lambda: checks.export(original, "x-output", "strict"))
expect_error("UNSUPPORTED_OPERATION", lambda: checks.parse("opaque", "x-output"))
checks.register("x-large", parser=lambda _: (_ for _ in ()).throw(AssertionError("must not parse oversized output")), exporter=lambda document, policy: {"output": "x" * 1048577, "losses": []})
expect_error("INPUT_TOO_LARGE", lambda: checks.export(original, "x-large"))
async def asynchronous_parser(text):
    return sdk.parse(text)
try:
    checks.register("x-async", parser=asynchronous_parser)
except TypeError:
    pass
else:
    raise AssertionError("Async callbacks must be rejected")
print("Python custom registry validation, rich round trips and preservation checks passed.")

expect_error("UNKNOWN_FORMAT", lambda: registry.parse("hi", "not-registered"))
for case in json.loads((root / "crates/unilyrics/tests/fixtures/conformance.json").read_text(encoding="utf-8")):
    if "error" in case:
        continue
    text = case["input"] if "input" in case else json.dumps(case["document"], ensure_ascii=False)
    assert registry.parse(text, case["format"]) == sdk.parse(text, case["format"]), case["name"]


for case in json.loads((root / "crates/unilyrics/tests/fixtures/profiles.json").read_text(encoding="utf-8")):
    before = json.dumps(case["request"], ensure_ascii=False)
    try:
        request = case["request"]
        if request["op"] == "project":
            result = sdk.project(request["document"], request.get("options"))
        else:
            result = sdk.export_profile(request["document"], request["format"], request.get("options"), request.get("policy", "compatible"))
        assert result == sdk.process(request)
    except UniLyricsError as error:
        assert error.code == case.get("error"), (case["name"], error)
    else:
        assert "error" not in case, case["name"]
        if "expected_document" in case:
            assert result["document"] == case["expected_document"], case["name"]
        else:
            assert json.loads(result["output"]) == case["output_document"], case["name"]
        assert [d["code"] for d in result.get("diagnostics", result.get("losses", []))] == case["codes"]
    assert json.dumps(case["request"], ensure_ascii=False) == before

for case in json.loads((root / "crates/unilyrics/tests/fixtures/profiles.json").read_text(encoding="utf-8")):
    if "error" in case or case["request"]["op"] != "project":
        continue
    request = case["request"]
    document = request["document"]
    before = json.dumps(document, ensure_ascii=False)
    preserved = sdk.export_preserved(document, "json", request.get("options"))
    assert json.loads(preserved["output"]) == case["expected_document"], case["name"]
    assert [d["code"] for d in preserved["losses"]] == case["codes"], case["name"]
    assert preserved["sidecar"]["document"] == document
    assert sdk.restore_preserved(preserved["output"], "json", preserved["sidecar"])["document"] == document
    for text, format, sidecar, code in (
        (preserved["output"] + "\n", "json", preserved["sidecar"], "SIDECAR_MISMATCH"),
        (preserved["output"], "plain", preserved["sidecar"], "SIDECAR_MISMATCH"),
        (preserved["output"], "json", {**preserved["sidecar"], "version": 2}, "UNSUPPORTED_SIDECAR_VERSION"),
    ):
        try:
            sdk.restore_preserved(text, format, sidecar)
            raise AssertionError("invalid sidecar accepted")
        except UniLyricsError as error:
            assert error.code == code
    assert json.dumps(document, ensure_ascii=False) == before
preservation_document = sdk.parse("[00:01]සිංහල 👩🏽‍💻", "lrc")["document"]
bundle = sdk.export_preserved(preservation_document, "plain")
assert bundle["losses"]
assert sdk.restore_preserved(bundle["output"], "plain", bundle["sidecar"])["document"] == preservation_document
print("Python preservation helper round trips, profiles and stale-sidecar checks passed.")

# Compact core JSON can fit while Python's normal request separators push it over.
size_document = sdk.parse("x", "plain")["document"]
seed = sdk.export_preserved(size_document, "plain")
restore_request = {"op":"restore_preserved", "input":seed["output"], "format":"plain", "sidecar":seed["sidecar"]}
seed_size = len(json.dumps(restore_request, ensure_ascii=False, separators=(",", ":")).encode())
count = 1 + (1_048_576 - seed_size) // 3
size_document["cues"][0]["segments"][0]["text"] = "x" * count
raw_bundle = sdk.process({"op":"export_preserved", "document":size_document, "format":"plain"})
restore_request = {"op":"restore_preserved", "input":raw_bundle["output"], "format":"plain", "sidecar":raw_bundle["sidecar"]}
assert len(json.dumps(restore_request, ensure_ascii=False).encode()) > 1_048_576
try:
    sdk.export_preserved(size_document, "plain")
    raise AssertionError("unrestorable encoded bundle returned")
except UniLyricsError as error:
    assert error.code == "INPUT_TOO_LARGE"
size_document["cues"][0]["segments"][0]["text"] = "x" * (count - 100)
bundle = sdk.export_preserved(size_document, "plain")
assert sdk.restore_preserved(bundle["output"], "plain", bundle["sidecar"])["document"] == size_document
