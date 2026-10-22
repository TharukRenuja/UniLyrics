"""Run permanently installed Android dex tools with clean diagnostics."""
from pathlib import Path
import shutil
import re
import subprocess


def d8_command(sdk: Path, build_tools: str, r8_jar: Path | None = None) -> list[str]:
    if r8_jar is not None:
        jar = r8_jar.resolve()
        if not jar.is_file():
            raise SystemExit(f"Missing installed R8 JAR: {jar}; nothing was downloaded")
        java = shutil.which("java")
        if not java:
            raise SystemExit("Missing permanent Java runtime; nothing was installed")
        return [java, "-cp", str(jar), "com.android.tools.r8.D8"]
    executable = sdk.resolve() / "build-tools" / build_tools / "d8"
    if not executable.is_file():
        raise SystemExit(f"Missing installed D8: {executable}; nothing was downloaded")
    return [str(executable)]


def run_d8(command, *arguments):
    completed = subprocess.run([*command, *map(str, arguments)], text=True,
                               capture_output=True, timeout=180)
    diagnostics = completed.stdout + completed.stderr
    print(diagnostics, end="", flush=True)
    completed.check_returncode()
    if re.search(r"(?m)^\s*(?:Warning|Error)(?:\s|:)", diagnostics):
        raise SystemExit("D8 diagnostics are not clean; use compatible installed D8/R8. Verification failed.")
