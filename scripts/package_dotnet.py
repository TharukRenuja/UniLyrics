"""Build a local NuGet package using installed .NET 10 and Rust; never publishes."""
import argparse
import os
from pathlib import Path
import struct
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--windows-native", type=Path, help="also bundle an existing Windows x64 engine")
    options = parser.parse_args()
    if sys.platform != "linux" or os.uname().machine != "x86_64":
        raise SystemExit("This local package builder currently runs on Linux x64")
    environment = dict(os.environ, DOTNET_CLI_HOME=str(ROOT / "target/dotnet-home"), DOTNET_CLI_TELEMETRY_OPTOUT="1")

    def run(*command):
        subprocess.run(list(map(str, command)), cwd=ROOT, env=environment, check=True)

    windows = options.windows_native.resolve() if options.windows_native else None
    if windows:
        content = windows.read_bytes()
        if windows.name != "unilyrics_ffi.dll" or content[:2] != b"MZ":
            raise SystemExit("Supply a Windows engine named unilyrics_ffi.dll")
        offset = struct.unpack_from("<I", content, 0x3c)[0]
        if content[offset:offset + 4] != b"PE\0\0" or struct.unpack_from("<H", content, offset + 4)[0] != 0x8664:
            raise SystemExit("The Windows DLL must target x64")
    run("cargo", "build", "--locked", "--offline", "--release", "-p", "unilyrics-ffi")
    native = ROOT / "target/release/libunilyrics_ffi.so"
    output = ROOT / "target/packages-dotnet"
    output.mkdir(exist_ok=True)
    config = ROOT / "target/dotnet-nuget.config"
    config.write_text('<configuration><packageSources><clear /></packageSources></configuration>\n')
    project = ROOT / "bindings/dotnet/UniLyrics/UniLyrics.csproj"
    run("dotnet", "restore", project, "--configfile", config)
    # Native asset properties are not tracked by NuGet's incremental pack inputs.
    # Remove this builder's previous outputs so a changed RID selection cannot leak through.
    (output / "UniLyrics.0.1.0.nupkg").unlink(missing_ok=True)
    for metadata in (project.parent / "obj/Release").glob("UniLyrics.0.1.0.nuspec"):
        metadata.unlink()
    run("dotnet", "pack", project, "--no-restore", "-c", "Release", "-o", output,
        f"-p:UniLyricsNativePath={native}", "-p:UniLyricsRid=linux-x64",
        f"-p:UniLyricsWindowsNativePath={windows or ''}")
    package = output / "UniLyrics.0.1.0.nupkg"
    with zipfile.ZipFile(package) as archive:
        assert archive.read("runtimes/linux-x64/native/libunilyrics_ffi.so") == native.read_bytes()
        assert "lib/net10.0/UniLyrics.dll" in archive.namelist()
        assert "licenses/rust/COPYRIGHT-library.html" in archive.namelist()
        expected_native = {"runtimes/linux-x64/native/libunilyrics_ffi.so"}
        if windows:
            expected_native.add("runtimes/win-x64/native/unilyrics_ffi.dll")
        assert {name for name in archive.namelist() if name.startswith("runtimes/")} == expected_native
        if windows:
            assert archive.read("runtimes/win-x64/native/unilyrics_ffi.dll") == windows.read_bytes()
    print(f"Local .NET package built: {package}; nothing published.")


if __name__ == "__main__":
    main()
