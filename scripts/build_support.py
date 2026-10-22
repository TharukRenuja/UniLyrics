"""Shared paths and process execution for local package builds."""
from pathlib import Path
import shlex
import subprocess
ROOT = Path(__file__).resolve().parents[1]


def run(arguments, cwd=ROOT, env=None):
    print("+ " + shlex.join(map(str, arguments)), flush=True)
    subprocess.run(list(map(str, arguments)), cwd=cwd, env=env, check=True)
