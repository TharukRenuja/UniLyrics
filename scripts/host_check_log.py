"""Timestamped, streamed logs for native host checks; no tool installation."""
from datetime import datetime, timezone
import json
from pathlib import Path
import platform
import subprocess
import sys
import traceback

ROOT = Path(__file__).resolve().parents[1]


def logged_check(name, check):
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    directory = ROOT / 'target' / 'host-checks' / name / datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    directory.mkdir(parents=True)
    log_path = directory / 'run.log'
    print(f'Log: {log_path}', flush=True)
    result = {'platform': platform.platform(), 'python': sys.version,
              'log': str(log_path), 'status': 'failed'}
    with log_path.open('w', encoding='utf-8') as log:
        def note(message):
            print(message, flush=True)
            log.write(message + '\n')
            log.flush()

        def run(command, cwd=ROOT, env=None):
            command = list(map(str, command))
            note('+ ' + repr(command))
            with subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace') as process:
                for line in process.stdout:
                    note(line.rstrip('\n'))
                code = process.wait()
            if code:
                raise subprocess.CalledProcessError(code, command)

        try:
            result['scope'] = check(run, directory)
            result['status'] = 'passed'
            note('PASS: ' + result['scope'])
        except Exception:
            note(traceback.format_exc())
        finally:
            (directory / 'result.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    return 0 if result['status'] == 'passed' else 1
