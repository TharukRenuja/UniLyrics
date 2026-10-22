package org.tharukrj.unilyrics;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;

/** Scoped synchronous callbacks shared by Java and Kotlin. Built-in parsing stays in Rust. */
public final class Registry {
    public interface Adapter {
        default boolean matches(String input) { return false; }
        default LyricsModel.ParseResult parse(String input) {
            throw new UniLyrics.LyricsException("UNSUPPORTED_OPERATION", "Custom format has no parser");
        }
        default LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) {
            throw new UniLyrics.LyricsException("UNSUPPORTED_OPERATION", "Custom format has no exporter");
        }
    }

    private final Map<String, Adapter> formats = new TreeMap<>();

    public void register(String name, Adapter adapter) {
        if (name == null || !name.matches("x-[a-z0-9_-]{1,62}")) {
            throw new UniLyrics.LyricsException("INVALID_CUSTOM_FORMAT", "Use a 3–64 character lowercase ASCII name in the x- namespace");
        }
        Objects.requireNonNull(adapter, "adapter");
        synchronized (formats) {
            if (formats.containsKey(name)) throw new UniLyrics.LyricsException("DUPLICATE_FORMAT", "Custom format is already registered");
            formats.put(name, adapter);
        }
    }

    private static void checkInput(String input) {
        Objects.requireNonNull(input, "input");
        if (!StandardCharsets.UTF_8.newEncoder().canEncode(input)) {
            throw new IllegalArgumentException("Input contains invalid Unicode");
        }
        if (input.getBytes(StandardCharsets.UTF_8).length > 1048576) {
            throw new UniLyrics.LyricsException("INPUT_TOO_LARGE", "Input exceeds 1 MiB");
        }
    }

    private Adapter adapter(String name) {
        synchronized (formats) { return formats.get(name); }
    }

    private static LyricsModel.Format builtin(String name) {
        try { return LyricsModel.Format.fromWireName(name); }
        catch (IllegalArgumentException error) { return null; }
    }

    public String detect(String input) {
        checkInput(input);
        String builtin = null;
        UniLyrics.LyricsException error = null;
        try { builtin = UniLyrics.detect(input); }
        catch (UniLyrics.LyricsException caught) { error = caught; }
        Map<String, Adapter> snapshot;
        synchronized (formats) { snapshot = new TreeMap<>(formats); }
        var matches = new ArrayList<String>();
        for (var entry : snapshot.entrySet()) {
            if (entry.getValue().matches(input)) matches.add(entry.getKey());
        }
        if (matches.size() > 1 || (!matches.isEmpty() && builtin != null && !builtin.equals("plain"))) {
            throw new UniLyrics.LyricsException("AMBIGUOUS_FORMAT", "Multiple formats match; choose explicitly");
        }
        if (!matches.isEmpty()) return matches.get(0);
        if (error != null) throw error;
        return builtin;
    }

    public LyricsModel.ParseResult parse(String input) { return parse(input, null); }
    public LyricsModel.ParseResult parse(String input, String format) {
        checkInput(input);
        String name = format == null ? detect(input) : format;
        Adapter adapter = adapter(name);
        if (adapter == null) {
            var builtin = builtin(name);
            if (builtin != null) return UniLyrics.parseTyped(input, builtin);
            throw new UniLyrics.LyricsException("UNKNOWN_FORMAT", "Format is not registered");
        }
        return checkedParse(adapter.parse(input));
    }

    private static LyricsModel.ParseResult checkedParse(LyricsModel.ParseResult result) {
        return LyricsModel.ParseResult.fromMap(UniLyrics.objectResult(UniLyrics.call(
            Json.request("op", "validate_custom_parse", "result", result.toMap()))));
    }

    public LyricsModel.ExportResult export(LyricsModel.Document document, String format) {
        return export(document, format, LyricsModel.Policy.COMPATIBLE);
    }
    public LyricsModel.ExportResult export(LyricsModel.Document document, String format, LyricsModel.Policy policy) {
        Objects.requireNonNull(format, "format");
        Objects.requireNonNull(policy, "policy");
        Adapter adapter = adapter(format);
        if (adapter == null) {
            var builtin = builtin(format);
            if (builtin != null) return UniLyrics.export(document, builtin, policy);
            throw new UniLyrics.LyricsException("UNKNOWN_FORMAT", "Format is not registered");
        }
        String canonical = UniLyrics.export(document, LyricsModel.Format.JSON, LyricsModel.Policy.STRICT).output();
        var original = Json.decode(canonical);
        var callbackDocument = LyricsModel.Document.fromMap(Json.decode(canonical));
        var generated = Objects.requireNonNull(adapter.export(callbackDocument, policy), "callback export result");
        checkInput(generated.output());
        var result = LyricsModel.ExportResult.fromMap(UniLyrics.objectResult(UniLyrics.call(
            Json.request("op", "validate_custom_export", "result", generated.toMap()))));
        LyricsModel.Document roundtrip = null;
        try { roundtrip = checkedParse(adapter.parse(result.output())).document(); }
        catch (RuntimeException error) { /* Failed reparsing is reported by shared verification. */ }
        // A missing reparse must remain explicit null on the wire.
        Map<String, Object> request = new java.util.LinkedHashMap<>();
        request.put("op", "verify_custom_export");
        request.put("document", original);
        request.put("result", result.toMap());
        request.put("roundtrip", roundtrip == null ? null : roundtrip.toMap());
        request.put("policy", policy.wireName());
        return LyricsModel.ExportResult.fromMap(UniLyrics.objectResult(UniLyrics.call(request)));
    }
}
