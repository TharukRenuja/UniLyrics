"""Check native profile entry points against the shared protocol goldens."""
import ctypes
import json
from pathlib import Path
import sys

library = ctypes.CDLL(str(Path(sys.argv[1]).resolve()))
library.unilyrics_project.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t]
library.unilyrics_export_profile.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p, ctypes.c_size_t]
for name in ("unilyrics_project", "unilyrics_export_profile"):
    getattr(library, name).restype = ctypes.c_void_p
library.unilyrics_free.argtypes = [ctypes.c_void_p]
library.unilyrics_free.restype = None


def reply(pointer):
    assert pointer
    try:
        return json.loads(ctypes.string_at(pointer))
    finally:
        library.unilyrics_free(pointer)


formats = {name: index for index, name in enumerate(("auto", "plain", "lrc", "enhanced_lrc", "json", "ttml", "ttml_lyrics", "srt", "webvtt"))}
root = Path(__file__).resolve().parents[2]
for case in json.loads((root / "crates/unilyrics/tests/fixtures/profiles.json").read_text()):
    request = case["request"]
    document = json.dumps(request["document"], ensure_ascii=False).encode()
    options = json.dumps(request["options"]).encode() if "options" in request else b""
    if request["op"] == "project":
        result = reply(library.unilyrics_project(document, len(document), options, len(options)))
    else:
        result = reply(library.unilyrics_export_profile(document, len(document), formats[request["format"]], request.get("policy") == "strict", options, len(options)))
    if "error" in case:
        assert not result["ok"] and result["error"]["code"] == case["error"], case["name"]
    else:
        assert result["ok"], (case["name"], result)
        value = result["result"]
        if "expected_document" in case:
            assert value["document"] == case["expected_document"], case["name"]
        else:
            assert json.loads(value["output"]) == case["output_document"], case["name"]
        assert [d["code"] for d in value.get("diagnostics", value.get("losses", []))] == case["codes"], case["name"]

valid = b'{"version":1,"cues":[]}'
for pointer, code in (
    (library.unilyrics_project(None, 1, None, 0), "INVALID_BUFFER"),
    (library.unilyrics_project(valid, len(valid), None, 1), "INVALID_BUFFER"),
    (library.unilyrics_project(valid, len(valid), b"\xff", 1), "INVALID_UTF8"),
    (library.unilyrics_project(valid, len(valid), b"{", 1), "INVALID_JSON"),
    (library.unilyrics_export_profile(valid, len(valid), 99, 0, None, 0), "INVALID_FORMAT"),
    (library.unilyrics_export_profile(valid, len(valid), 4, 2, None, 0), "INVALID_POLICY"),
):
    assert reply(pointer)["error"]["code"] == code
assert reply(library.unilyrics_project(valid, len(valid), None, 0))["ok"]
print("Native profile goldens, buffers, policy and ownership checks passed.")

library.unilyrics_export_preserved.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int32, ctypes.c_void_p, ctypes.c_size_t]
library.unilyrics_restore_preserved.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int32, ctypes.c_void_p, ctypes.c_size_t]
library.unilyrics_export_preserved.restype = ctypes.c_void_p
library.unilyrics_restore_preserved.restype = ctypes.c_void_p
for case in json.loads((root / "crates/unilyrics/tests/fixtures/profiles.json").read_text()):
    request = case["request"]
    if "error" in case or request["op"] != "project":
        continue
    document = json.dumps(request["document"], ensure_ascii=False).encode()
    options = json.dumps(request.get("options", {})).encode()
    exported = reply(library.unilyrics_export_preserved(document, len(document), 4, options, len(options)))
    assert exported["ok"], exported
    bundle = exported["result"]
    assert json.loads(bundle["output"]) == case["expected_document"], case["name"]
    assert [d["code"] for d in bundle["losses"]] == case["codes"], case["name"]
    sidecar = json.dumps(bundle["sidecar"], ensure_ascii=False).encode()
    output = bundle["output"].encode()
    restored = reply(library.unilyrics_restore_preserved(output, len(output), 4, sidecar, len(sidecar)))
    assert restored["ok"] and restored["result"]["document"] == request["document"], case["name"]
    stale = output + b"\n"
    assert reply(library.unilyrics_restore_preserved(stale, len(stale), 4, sidecar, len(sidecar)))["error"]["code"] == "SIDECAR_MISMATCH"
    assert reply(library.unilyrics_restore_preserved(output, len(output), 1, sidecar, len(sidecar)))["error"]["code"] == "SIDECAR_MISMATCH"
    invalid = json.dumps({**bundle["sidecar"], "version": 2}).encode()
    assert reply(library.unilyrics_restore_preserved(output, len(output), 4, invalid, len(invalid)))["error"]["code"] == "UNSUPPORTED_SIDECAR_VERSION"

for pointer, code in (
    (library.unilyrics_export_preserved(None, 1, 4, None, 0), "INVALID_BUFFER"),
    (library.unilyrics_export_preserved(valid, len(valid), 99, None, 0), "INVALID_FORMAT"),
    (library.unilyrics_restore_preserved(None, 1, 1, valid, len(valid)), "INVALID_BUFFER"),
    (library.unilyrics_restore_preserved(None, 0, 1, None, 1), "INVALID_BUFFER"),
    (library.unilyrics_restore_preserved(None, 0, 1, b"\xff", 1), "INVALID_UTF8"),
    (library.unilyrics_restore_preserved(None, 0, 1, b"{", 1), "INVALID_JSON"),
    (library.unilyrics_restore_preserved(None, 0, 99, valid, len(valid)), "INVALID_FORMAT"),
):
    assert reply(pointer)["error"]["code"] == code
empty = reply(library.unilyrics_export_preserved(valid, len(valid), 1, None, 0))["result"]
sidecar = json.dumps(empty["sidecar"]).encode()
assert reply(library.unilyrics_restore_preserved(None, 0, 1, sidecar, len(sidecar)))["ok"]
print("Native preservation snapshots, stale pairing, buffers and ownership checks passed.")

# Native adapters compact the restore request, so a compact near-limit pair works.
size_document = {"version":1,"cues":[{"id":"cue","segments":[{"text":"x"}]}]}
seed = json.dumps(size_document).encode()
seed_bundle = reply(library.unilyrics_export_preserved(seed, len(seed), 1, None, 0))["result"]
seed_request = {"op":"restore_preserved", "input":seed_bundle["output"], "format":"plain", "sidecar":seed_bundle["sidecar"]}
count = 1 + (1048576 - len(json.dumps(seed_request, separators=(",", ":")).encode())) // 3
size_document["cues"][0]["segments"][0]["text"] = "x" * count
encoded = json.dumps(size_document).encode()
near_limit = reply(library.unilyrics_export_preserved(encoded, len(encoded), 1, None, 0))
assert near_limit["ok"]
output = near_limit["result"]["output"].encode()
sidecar = json.dumps(near_limit["result"]["sidecar"]).encode()
assert reply(library.unilyrics_restore_preserved(output, len(output), 1, sidecar, len(sidecar)))["ok"]
size_document["cues"][0]["segments"][0]["text"] += "x"
encoded = json.dumps(size_document).encode()
assert reply(library.unilyrics_export_preserved(encoded, len(encoded), 1, None, 0))["error"]["code"] == "INPUT_TOO_LARGE"
