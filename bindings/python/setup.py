"""Optionally include a prebuilt ctypes library, with a platform wheel tag."""
import os
from pathlib import Path
import shutil
from setuptools import setup
from setuptools.command.build_py import build_py
from setuptools.command.bdist_wheel import bdist_wheel

native = os.environ.get("UNILYRICS_NATIVE_LIBRARY")


class BuildPython(build_py):
    def run(self):
        super().run()
        local = Path(__file__).resolve().parent
        license_root = local if (local / "licenses").is_dir() else local.parent.parent
        if (license_root / "licenses").is_dir():
            shutil.copytree(license_root / "licenses", Path(self.build_lib) / "unilyrics/licenses", dirs_exist_ok=True)
            shutil.copy2(license_root / "THIRD_PARTY_NOTICES.md", Path(self.build_lib) / "unilyrics/THIRD_PARTY_NOTICES.md")
        if native:
            source = Path(native).resolve(strict=True)
            destination = Path(self.build_lib) / "unilyrics" / "_native"
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination / source.name)


class Wheel(bdist_wheel):
    def finalize_options(self):
        super().finalize_options()
        if native:
            self.root_is_pure = False

    def get_tag(self):
        python, abi, platform = super().get_tag()
        return ("py3", "none", platform) if native else (python, abi, platform)


setup(cmdclass={"build_py": BuildPython, "bdist_wheel": Wheel})
