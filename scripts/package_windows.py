"""Build local Windows native/Python SDK packages."""
import argparse
import base64
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import struct
import sys
import tempfile
import tomllib
import zipfile

from package_docs import copy_examples, copy_public_docs

from build_support import ROOT
TARGET = "x86_64-pc-windows-gnu"


def run(*arguments, **options):
    subprocess.run(list(map(str, arguments)), cwd=options.pop("cwd", ROOT),
                   check=True, **options)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    for tool in ["cargo", "cmake", "x86_64-w64-mingw32-gcc"]:
        if not shutil.which(tool):
            raise SystemExit(f"Missing permanent tool: {tool}; nothing was installed")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    run("cargo", "build", "--locked", "--offline", "--release", "-p", "unilyrics-ffi", "--target", TARGET)
    native = ROOT / "target" / TARGET / "release"
    output = ROOT / "target/packages-windows"
    output.mkdir(exist_ok=True)
    (output / "manifest.json").unlink(missing_ok=True)
    with tempfile.TemporaryDirectory(prefix="unilyrics-windows-") as temporary:
        scratch = Path(temporary)
        sdk = scratch / "unilyrics-native"
        run("cmake", "-S", ROOT / "bindings/c", "-B", scratch / "install-build",
            "-DCMAKE_SYSTEM_NAME=Windows", f"-DUNILYRICS_NATIVE_DIR={native}",
            "-DCMAKE_INSTALL_LIBDIR=lib")
        run("cmake", "--install", scratch / "install-build", "--prefix", sdk)
        shutil.copytree(ROOT / "bindings/go", sdk / "bindings/go")
        shutil.copytree(ROOT / "bindings/android", sdk / "bindings/android")
        copy_public_docs(ROOT, sdk)
        shutil.copytree(ROOT / "spec", sdk / "spec")
        copy_examples(ROOT, sdk)
        archive = output / f"unilyrics-native-{version}-windows-x86_64-mingw.zip"
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(sdk.rglob("*")):
                if path.is_file():
                    bundle.write(path, path.relative_to(scratch))
        python_source = scratch / "python"
        shutil.copytree(ROOT / "bindings/python", python_source,
                        ignore=shutil.ignore_patterns("build", "*.egg-info", "__pycache__"))
        shutil.copytree(ROOT / "licenses", python_source / "licenses")
        shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", python_source)
        python_environment = dict(os.environ, UNILYRICS_NATIVE_LIBRARY=str(native / "unilyrics_ffi.dll"))
        run(sys.executable, "-m", "pip", "wheel", "--no-cache-dir", "--no-build-isolation", "--no-deps", "--no-index",
            "--config-settings=--build-option=--plat-name=win_amd64", "--wheel-dir", scratch,
            python_source, env=python_environment)
        wheel = scratch / f"unilyrics-{version}-py3-none-win_amd64.whl"
        with zipfile.ZipFile(wheel) as bundle:
            names = bundle.namelist()
            native_name, = [name for name in names if name.endswith("unilyrics/_native/unilyrics_ffi.dll")]
            payload = bundle.read(native_name)
            assert payload == (native / "unilyrics_ffi.dll").read_bytes()
            with zipfile.ZipFile(archive) as sdk_archive:
                assert sdk_archive.read("unilyrics-native/bin/unilyrics_ffi.dll") == payload
            assert payload[:2] == b"MZ"
            pe = struct.unpack_from("<I", payload, 0x3c)[0]
            assert payload[pe:pe + 4] == b"PE\0\0" and struct.unpack_from("<H", payload, pe + 4)[0] == 0x8664
            assert not any(name.endswith((".so", ".dylib")) for name in names)
            assert any(name.endswith("unilyrics/py.typed") for name in names)
            wheel_info, = [name for name in names if name.endswith(".dist-info/WHEEL")]
            assert "Root-Is-Purelib: false" in bundle.read(wheel_info).decode()
            assert "Tag: py3-none-win_amd64" in bundle.read(wheel_info).decode()
            metadata, = [name for name in names if name.endswith(".dist-info/METADATA")]
            assert "Requires-Python: >=3.9" in bundle.read(metadata).decode()
            record, = [name for name in names if name.endswith(".dist-info/RECORD")]
            rows = list(csv.reader(io.StringIO(bundle.read(record).decode())))
            assert {row[0] for row in rows} == set(names)
            for name, digest, size in rows:
                if name == record:
                    assert digest == size == ""
                else:
                    content = bundle.read(name)
                    expected = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode()
                    assert digest == "sha256=" + expected and int(size) == len(content), name
        extracted_python = scratch / "python-installed"
        run(sys.executable, "-m", "pip", "install", "--no-cache-dir", "--no-index", "--no-deps", "--only-binary=:all:",
            "--platform", "win_amd64", "--python-version", "3.9", "--target", extracted_python, wheel)
        assert (extracted_python / "unilyrics/_native/unilyrics_ffi.dll").read_bytes() == payload
        assert (extracted_python / "unilyrics/py.typed").is_file()
        shutil.copy2(wheel, output / wheel.name)
        print("Windows Python wheel tag, PE payload, RECORD hashes and cross-install layout pass; Python runtime remains unverified.")
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        manifest = {"version": version, "platform": "windows-x86_64-mingw",
                    "sha256": {archive.name: digest, wheel.name: hashlib.sha256(wheel.read_bytes()).hexdigest()}}
        (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Local Windows native SDK: {archive}; nothing published.")


if __name__ == "__main__":
    main()
