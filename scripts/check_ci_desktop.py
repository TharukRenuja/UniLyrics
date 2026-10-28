"""Exercise native Rust, Python and .NET consumers on a hosted desktop runner."""
import os
import sys

from ci_protocol import ROOT, check_protocol
from host_check_log import logged_check


def check(run, directory):
    run(['cargo', 'test', '--locked', '--offline', '--workspace'])
    run(['cargo', 'build', '--locked', '--offline', '-p', 'unilyrics-ffi'])
    name = {'win32': 'unilyrics_ffi.dll', 'darwin': 'libunilyrics_ffi.dylib',
            'linux': 'libunilyrics_ffi.so'}[sys.platform]
    library = ROOT / 'target/debug' / name
    run([sys.executable, '-B', 'bindings/python/test_conformance.py', library])
    project = ROOT / 'bindings/dotnet/Tests/Tests.csproj'
    config = directory / 'NuGet.Config'
    config.write_text('<configuration><packageSources><clear /></packageSources></configuration>\n')
    environment = dict(os.environ, DOTNET_CLI_TELEMETRY_OPTOUT='1',
                       DOTNET_CLI_HOME=str(directory / 'dotnet-home'))
    run(['dotnet', 'restore', project, '--configfile', config], env=environment)
    run(['dotnet', 'build', project, '--no-restore', '-c', 'Release'], env=environment)
    binary = project.parent / 'bin/Release/net10.0/Tests.dll'
    run(['dotnet', binary, ROOT, library], env=environment)
    check_protocol(['dotnet', str(binary), '--protocol', str(library)])
    return 'Native Rust, Python and .NET source consumers; installed packages are not checked.'


if __name__ == '__main__':
    sys.exit(logged_check('desktop', check))
