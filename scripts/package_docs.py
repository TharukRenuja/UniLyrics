"""Explicit public documentation list shared by native package builders."""
from pathlib import Path
import shutil

PUBLIC_DOCS = ('API.md', 'COMPATIBILITY.md', 'DATA_MODEL.md', 'EXTENSIONS.md', 'FORMATS.md', 'GETTING_STARTED.md', 'INSTALLATION.md', 'LANGUAGES.md', 'NORMALIZATION.md', 'PLATFORMS.md', 'PLAYER_INTEGRATION.md', 'PRESERVATION.md', 'PROFILES.md', 'README.md', 'SRT.md', 'TROUBLESHOOTING.md', 'TTML.md', 'WEBVTT.md')


def copy_public_docs(root: Path, sdk: Path) -> None:
    """New files are private to packaging until explicitly added above."""
    destination = sdk / "docs"
    destination.mkdir()
    for name in PUBLIC_DOCS:
        shutil.copy2(root / "docs" / name, destination / name)
    shutil.copy2(root / "README.md", sdk / "README.md")
    shutil.copy2(root / "LICENSE", sdk / "LICENSE")
    shutil.copy2(root / "THIRD_PARTY_NOTICES.md", sdk / "THIRD_PARTY_NOTICES.md")
    shutil.copytree(root / "assets", sdk / "assets")
    shutil.copytree(root / "licenses", sdk / "licenses")


def copy_examples(root: Path, sdk: Path) -> None:
    """Include source examples without local SDK paths, IDE state or build outputs."""
    shutil.copytree(root / "examples", sdk / "examples", ignore=shutil.ignore_patterns(
        "__pycache__", "*.pyc", ".gradle", ".kotlin", ".idea", "build", "bin", "obj", "local.properties"))
