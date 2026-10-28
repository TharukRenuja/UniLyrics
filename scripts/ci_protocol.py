"""Shared SDK protocol assertions used by CI consumers."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def check_protocol(command):
    cases = json.loads((ROOT / "crates/unilyrics/tests/fixtures/conformance.json").read_text(encoding="utf-8"))
    checks = []
    for case in cases:
        text = case["input"] if "input" in case else json.dumps(case["document"], ensure_ascii=False)
        checks.append((case, "parse", {"op": "parse", "input": text, "format": case["format"]}))
        for index, expected in enumerate(case.get("exports", [])):
            checks.append((case, "export_fixture_" + str(index), {"op":"export", "document":case["document"], "format":expected["format"], "policy":expected["policy"]}))
        if "features" in case:
            for operation in ["inspect", "validate"]:
                checks.append((case, operation, {"op": operation, "document": case["document"]}))
            for format, policy in [("json", "strict"), ("plain", "compatible"), ("plain", "strict")]:
                checks.append((case, format + "_" + policy, {"op": "export", "document": case["document"], "format": format, "policy": policy}))
    requests = [json.dumps(request, ensure_ascii=False) for _, _, request in checks]
    completed = subprocess.run(list(map(str, command)), input="\n".join(requests) + "\n", text=True, encoding="utf-8", stdout=subprocess.PIPE, check=True)
    responses = completed.stdout.splitlines()
    assert len(responses) == len(checks), (command, completed.stdout)
    for (case, operation, _), line in zip(checks, responses):
        response = json.loads(line)
        expected_error = "LOSSY_CONVERSION" if operation == "plain_strict" else case.get("error")
        if expected_error:
            assert response["ok"] is False and response["error"]["code"] == expected_error, (command, case, response)
            continue
        assert response["ok"] is True, response
        result = response["result"]
        if operation.startswith("export_fixture_"):
            expected = case["exports"][int(operation.removeprefix("export_fixture_"))]
            assert result["output"] == expected["output"] and result["losses"] == [], (command, case, result)
        elif operation == "parse":
            cues = result["document"]["cues"]
            assert ["".join(segment["text"] for segment in cue["segments"]) for cue in cues] == case["text"], (command, case)
            assert [cue["start_us"] for cue in cues] == case["starts"], (command, case)
            if "document" in case:
                assert result["document"] == case["document"], (command, case)
        elif operation == "inspect":
            assert result == case["features"], (command, result)
        elif operation == "validate":
            assert result == {"valid": True}, (command, result)
        elif operation == "json_strict":
            assert json.loads(result["output"]) == case["document"], (command, result)
        else:
            assert {loss["code"] for loss in result["losses"]} == set(case["loss_codes"]), (command, result)
    print("Shared protocol fixtures passed: " + str(command[0]), flush=True)
    check_normalization_protocol(command)
    check_profile_protocol(command)
    check_preservation_protocol(command)


def check_normalization_protocol(command):
    cases = json.loads((ROOT / "crates/unilyrics/tests/fixtures/normalization.json").read_text(encoding="utf-8"))
    requests = [{"op":"normalize", "document":case["document"], **({"offset_us":case["offset_us"]} if "offset_us" in case else {})} for case in cases]
    completed = subprocess.run(list(map(str, command)), input="\n".join(json.dumps(r, ensure_ascii=False) for r in requests) + "\n", text=True, encoding="utf-8", stdout=subprocess.PIPE, check=True)
    responses = completed.stdout.splitlines()
    assert len(responses) == len(cases), (command, completed.stdout)
    for case, line in zip(cases, responses):
        reply = json.loads(line)
        if "error" in case:
            assert reply["ok"] is False and reply["error"]["code"] == case["error"], (command, case["name"], reply)
        else:
            assert reply["ok"] is True, reply
            assert reply["result"]["document"] == case["expected_document"], (command, case["name"], reply)
            assert [d["code"] for d in reply["result"]["diagnostics"]] == case["diagnostic_codes"]
    print("Shared normalization fixtures passed: " + str(command[0]), flush=True)


def check_profile_protocol(command):
    cases = json.loads((ROOT / "crates/unilyrics/tests/fixtures/profiles.json").read_text(encoding="utf-8"))
    completed = subprocess.run(list(map(str, command)), input="\n".join(json.dumps(c["request"], ensure_ascii=False) for c in cases) + "\n", text=True, encoding="utf-8", stdout=subprocess.PIPE, check=True)
    responses = completed.stdout.splitlines()
    assert len(responses) == len(cases)
    for case, line in zip(cases, responses):
        reply = json.loads(line)
        if "error" in case:
            assert reply["ok"] is False and reply["error"]["code"] == case["error"], (case["name"], reply)
            continue
        assert reply["ok"] is True, reply
        result = reply["result"]
        if "expected_document" in case:
            assert result["document"] == case["expected_document"], (case["name"], result)
        else:
            assert json.loads(result["output"]) == case["output_document"], (case["name"], result)
        assert [d["code"] for d in result.get("diagnostics", result.get("losses", []))] == case["codes"]
    print("Shared profile fixtures passed: " + str(command[0]), flush=True)


def check_preservation_protocol(command):
    cases = json.loads((ROOT / "crates/unilyrics/tests/fixtures/profiles.json").read_text(encoding="utf-8"))
    source = cases[0]["request"]["document"]
    requests = [{"op":"export_preserved", "document":source, "format":fmt, "options":{"profile":profile}}
                for fmt, profile in [("json", "full"), ("plain", "plain"), ("json", "translated")]]
    def exchange(requests):
        replies = subprocess.run([str(part) for part in command], input="\n".join(json.dumps(r, ensure_ascii=False) for r in requests) + "\n", stdout=subprocess.PIPE, text=True, encoding="utf-8", check=True)
        results = [json.loads(line) for line in replies.stdout.splitlines()]
        assert len(results) == len(requests), command
        return results
    exported = exchange(requests)
    restores = []
    for request, reply in zip(requests, exported):
        assert reply["ok"], (command, reply)
        bundle = reply["result"]
        assert bundle["sidecar"]["document"] == source, command
        restores.append({"op":"restore_preserved", "input":bundle["output"], "format":request["format"], "sidecar":bundle["sidecar"]})
    for reply in exchange(restores):
        assert reply["ok"] and reply["result"]["document"] == source, (command, reply)
        assert reply["result"]["diagnostics"][0]["code"] == "SIDECAR_RESTORED"
    original = restores[0]
    invalid = [dict(original, input=original["input"]+"\n"),
               dict(original, sidecar={**original["sidecar"], "version":2}),
               dict(original, sidecar={**original["sidecar"], "unexpected":True})]
    for reply, code in zip(exchange(invalid), ["SIDECAR_MISMATCH", "UNSUPPORTED_SIDECAR_VERSION", "INVALID_REQUEST"]):
        assert not reply["ok"] and reply["error"]["code"] == code, (command, reply)
    print("Shared preservation protocol checks passed: " + str(command[0]), flush=True)


def check_cpp_registry(binary):
    cases = [case for case in json.loads((ROOT / "crates/unilyrics/tests/fixtures/conformance.json").read_text(encoding="utf-8")) if "error" not in case]
    requests = [{"op":"parse", "input":case["input"] if "input" in case else json.dumps(case["document"], ensure_ascii=False), "format":case["format"]} for case in cases]
    replies = subprocess.run([str(binary), "--protocol"], input="\n".join(json.dumps(r, ensure_ascii=False) for r in requests) + "\n", stdout=subprocess.PIPE, text=True, encoding="utf-8", check=True)
    documents = [json.loads(line)["result"]["document"] for line in replies.stdout.splitlines()]
    assert len(documents) == len(cases), "C++ registry must cover every valid shared fixture"
    custom = subprocess.run([str(binary), "--custom-protocol"], input="\n".join(json.dumps(d, ensure_ascii=False) for d in documents) + "\n", stdout=subprocess.PIPE, text=True, encoding="utf-8", check=True)
    lines = custom.stdout.splitlines()
    assert len(lines) == len(documents) * 3
    for index, document in enumerate(documents):
        parsed, detected, exported = [json.loads(line) for line in lines[index * 3:index * 3 + 3]]
        assert parsed["ok"] and parsed["result"]["document"] == document
        assert detected == {"ok":True, "result":"x-wrapped"}
        assert exported["ok"] and exported["result"]["losses"] == []
        assert json.loads(exported["result"]["output"][len("CUSTOM:"):]) == document
    print("C++ custom registry passed the full valid shared corpus.", flush=True)
