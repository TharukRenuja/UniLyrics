"""Local UniLyrics binding over the shared native library. Schema is experimental."""
import ctypes
import json
import os
import re
from inspect import iscoroutinefunction
import sys
from pathlib import Path
from typing import Any, Callable, Dict, Optional, Tuple, Union, cast, get_args

from .types import (
    Agent, Attributes, Auxiliary, Cue, Diagnostic, Document, ExportResult,
    Features, Format, Json, Metadata, ParseResult, Policy, Segment, ValidationResult,
    Profile, ProjectionFeature, ProjectionOptions, PreservationSidecar, PreservedExportResult,
)

__all__ = [
    "UniLyrics", "UniLyricsError", "Registry", "Agent", "Attributes", "Auxiliary", "Cue",
    "Diagnostic", "Document", "ExportResult", "Features", "Format", "Json",
    "Metadata", "ParseResult", "Policy", "Segment", "ValidationResult",
    "Profile", "ProjectionFeature", "ProjectionOptions", "PreservationSidecar", "PreservedExportResult",
]


class UniLyricsError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(f"{code}: {message}")
        self.code = code


class UniLyrics:
    def __init__(self, library_path: Optional[Union[str, os.PathLike[str]]] = None) -> None:
        if library_path is None:
            library_path = os.environ.get("UNILYRICS_LIBRARY")
        if library_path is None:
            name = {"win32": "unilyrics_ffi.dll", "darwin": "libunilyrics_ffi.dylib"}.get(sys.platform, "libunilyrics_ffi.so")
            library_path = Path(__file__).parent / "_native" / name
            if not library_path.is_file():
                raise FileNotFoundError("No bundled UniLyrics engine; pass library_path or set UNILYRICS_LIBRARY")
        self._library = ctypes.CDLL(str(Path(library_path).resolve()))
        self._library.unilyrics_process.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        self._library.unilyrics_process.restype = ctypes.c_void_p
        self._library.unilyrics_free.argtypes = [ctypes.c_void_p]
        self._library.unilyrics_free.restype = None

    def process(self, request: Any) -> Any:
        data = json.dumps(request, ensure_ascii=False, allow_nan=False).encode("utf-8")
        response = self._library.unilyrics_process(data, len(data))
        if not response:
            raise RuntimeError("UniLyrics returned no response")
        try:
            result = json.loads(ctypes.string_at(response))
        finally:
            self._library.unilyrics_free(response)
        if not result["ok"]:
            error = result["error"]
            raise UniLyricsError(error["code"], error["message"])
        return result["result"]

    def parse(self, text: str, format: Optional[Format] = None) -> ParseResult:
        return cast(ParseResult, self.process({"op": "parse", "input": text, "format": format}))

    def detect(self, text: str) -> Format:
        return cast(Format, self.process({"op": "detect", "input": text}))

    def export(self, document: Document, format: Format, policy: Policy = "compatible") -> ExportResult:
        return cast(ExportResult, self.process({"op": "export", "document": document, "format": format, "policy": policy}))

    def validate(self, document: Document) -> ValidationResult:
        return cast(ValidationResult, self.process({"op": "validate", "document": document}))

    def inspect(self, document: Document) -> Features:
        return cast(Features, self.process({"op": "inspect", "document": document}))

    def export_preserved(self, document: Document, format: Format,
                         options: Optional[ProjectionOptions] = None) -> PreservedExportResult:
        result = cast(PreservedExportResult, self.process({"op": "export_preserved", "document": document, "format": format,
                                                         "options": {} if options is None else options}))
        restore = {"op": "restore_preserved", "input": result["output"], "format": format, "sidecar": result["sidecar"]}
        if len(json.dumps(restore, ensure_ascii=False, allow_nan=False).encode("utf-8")) > 1_048_576:
            raise UniLyricsError("INPUT_TOO_LARGE", "Encoded preservation restore request exceeds 1 MiB")
        return result

    def restore_preserved(self, text: str, format: Format, sidecar: PreservationSidecar) -> ParseResult:
        return cast(ParseResult, self.process({"op": "restore_preserved", "input": text, "format": format, "sidecar": sidecar}))

    def project(self, document: Document, options: Optional[ProjectionOptions] = None) -> ParseResult:
        return cast(ParseResult, self.process({"op": "project", "document": document, "options": {} if options is None else options}))

    def export_profile(self, document: Document, format: Format, options: Optional[ProjectionOptions] = None,
                       policy: Policy = "compatible") -> ExportResult:
        return cast(ExportResult, self.process({"op": "export_profile", "document": document, "format": format,
                                                "options": {} if options is None else options, "policy": policy}))

    def normalize(self, document: Document, offset_us: int = 0) -> ParseResult:
        return cast(ParseResult, self.process({"op": "normalize", "document": document, "offset_us": offset_us}))


class Registry:
    """Scoped synchronous callbacks; the shared engine validates results and export losses."""

    def __init__(self, engine: UniLyrics) -> None:
        self._engine = engine
        self._formats: Dict[str, Tuple[
            Optional[Callable[[str], ParseResult]],
            Optional[Callable[[Document, Policy], ExportResult]],
            Optional[Callable[[str], bool]],
        ]] = {}

    def register(self, name: str, *, parser: Optional[Callable[[str], ParseResult]] = None,
                 exporter: Optional[Callable[[Document, Policy], ExportResult]] = None,
                 detector: Optional[Callable[[str], bool]] = None) -> None:
        if not isinstance(name, str) or re.fullmatch(r"x-[a-z0-9_-]{1,62}", name) is None:
            raise UniLyricsError("INVALID_CUSTOM_FORMAT", "Use a 3–64 character lowercase ASCII name in the x- namespace.")
        if name in self._formats:
            raise UniLyricsError("DUPLICATE_FORMAT", "Custom format is already registered.")
        for callback in (parser, exporter, detector):
            if callback is not None and (not callable(callback) or iscoroutinefunction(callback) or iscoroutinefunction(getattr(callback, "__call__", None))):
                raise TypeError("Custom callbacks must be synchronous callables")
        self._formats[name] = (parser, exporter, detector)

    @staticmethod
    def _check_input(text: str) -> None:
        if not isinstance(text, str):
            raise TypeError("Input must be a string")
        if len(text.encode("utf-8")) > 1_048_576:
            raise UniLyricsError("INPUT_TOO_LARGE", "Input exceeds 1 MiB.")

    def detect(self, text: str) -> str:
        self._check_input(text)
        builtin = None
        error = None
        try:
            builtin = self._engine.detect(text)
        except UniLyricsError as caught:
            error = caught
        matches = []
        for name, (_, _, detector) in self._formats.items():
            if detector is not None:
                matched = detector(text)
                if not isinstance(matched, bool):
                    raise TypeError("Custom detector must return a boolean")
                if matched:
                    matches.append(name)
        if len(matches) > 1 or (matches and builtin is not None and builtin != "plain"):
            raise UniLyricsError("AMBIGUOUS_FORMAT", "Multiple formats match; choose explicitly.")
        if matches:
            return matches[0]
        if error is not None:
            raise error
        return cast(str, builtin)

    def parse(self, text: str, format: Optional[str] = None) -> ParseResult:
        self._check_input(text)
        name = self.detect(text) if format is None else format
        adapter = self._formats.get(name)
        if adapter is None:
            if name not in get_args(Format):
                raise UniLyricsError("UNKNOWN_FORMAT", "Format is not registered.")
            return self._engine.parse(text, cast(Format, name))
        parser = adapter[0]
        if parser is None:
            raise UniLyricsError("UNSUPPORTED_OPERATION", "Custom format has no parser.")
        return cast(ParseResult, self._engine.process({"op": "validate_custom_parse", "result": parser(text)}))

    def export(self, document: Document, format: str, policy: Policy = "compatible") -> ExportResult:
        adapter = self._formats.get(format)
        if adapter is None:
            if format not in get_args(Format):
                raise UniLyricsError("UNKNOWN_FORMAT", "Format is not registered.")
            return self._engine.export(document, cast(Format, format), policy)
        if policy not in ("strict", "compatible"):
            raise UniLyricsError("INVALID_POLICY", "Unknown export policy.")
        canonical = self._engine.export(document, "json", "strict")["output"]
        parser, exporter, _ = adapter
        if exporter is None:
            raise UniLyricsError("UNSUPPORTED_OPERATION", "Custom format has no exporter.")
        result = self._engine.process({"op": "validate_custom_export", "result": exporter(json.loads(canonical), policy)})
        roundtrip = None
        if parser is not None:
            try:
                roundtrip = self._engine.process({"op": "validate_custom_parse", "result": parser(result["output"])})["document"]
            except Exception:
                pass  # Failed callback reparsing is reported by shared verification.
        return cast(ExportResult, self._engine.process({"op": "verify_custom_export", "document": json.loads(canonical), "result": result, "roundtrip": roundtrip, "policy": policy}))
