"""Exercise a local NuGet SDK in an isolated app on its actual .NET host."""
import argparse
import os
from pathlib import Path
import shutil
import tempfile
import xml.etree.ElementTree as ET
from build_support import ROOT, run
from ci_protocol import check_protocol


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--workdir", type=Path, help="retain build files inside the host check log directory")
    options = parser.parse_args()
    environment = dict(os.environ, DOTNET_CLI_HOME=str(ROOT / "target/dotnet-home"), DOTNET_CLI_TELEMETRY_OPTOUT="1")
    package = options.package.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="unilyrics-dotnet-consumer-") as temporary:
        directory = options.workdir.resolve() if options.workdir else Path(temporary)
        directory.mkdir(parents=True, exist_ok=True)
        consumer = directory / "Consumer.csproj"
        consumer.write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings><TreatWarningsAsErrors>true</TreatWarningsAsErrors></PropertyGroup><ItemGroup><PackageReference Include="UniLyrics" Version="0.1.0" /></ItemGroup></Project>')
        shutil.copy2(ROOT / "bindings/dotnet/Tests/Program.cs", directory / "Program.cs")
        tree = ET.Element("configuration")
        sources = ET.SubElement(tree, "packageSources")
        ET.SubElement(sources, "clear")
        ET.SubElement(sources, "add", key="local", value=str(package.parent))
        local_config = directory / "NuGet.Config"
        ET.ElementTree(tree).write(local_config)
        installed_env = dict(environment)
        installed_env.pop("UNILYRICS_LIBRARY", None)
        run(["dotnet", "restore", consumer, "--configfile", local_config,
             "--packages", directory / "packages"], env=installed_env)
        run(["dotnet", "build", consumer, "--no-restore", "-c", "Release"], env=installed_env)
        installed = directory / "bin/Release/net10.0/Consumer.dll"
        run(["dotnet", installed, ROOT, "-"], env=installed_env)
        # The protocol subprocess must also resolve the bundled library.
        previous = os.environ.pop("UNILYRICS_LIBRARY", None)
        try:
            check_protocol(["dotnet", installed, "--protocol", "-"])
        finally:
            if previous is not None:
                os.environ["UNILYRICS_LIBRARY"] = previous
    print("Isolated installed NuGet consumer and shared protocol passed.")


if __name__ == "__main__":
    main()
