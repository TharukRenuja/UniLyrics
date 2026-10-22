package org.tharukrj.unilyrics;

import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.LinkedHashMap;
import java.util.List;

/** Native lyric operations using ordinary Java maps/lists. Parsing stays in Rust. */
public final class UniLyrics {
    private UniLyrics() {}
    private static boolean bundledLoaded;

    /** Load the native libraries bundled in a platform-specific SDK jar. */
    public static synchronized void load() {
        if (bundledLoaded) return;
        String os = System.getProperty("os.name").toLowerCase(java.util.Locale.ROOT);
        String arch = System.getProperty("os.arch");
        if (!os.equals("linux") || !(arch.equals("amd64") || arch.equals("x86_64"))) {
            throw new UnsupportedOperationException("This experimental bundled SDK targets Linux x86_64; use load(path) for another build");
        }
        String prefix = "/org/tharukrj/unilyrics/native/linux-x86_64/";
        try {
            java.nio.file.Path directory = java.nio.file.Files.createTempDirectory("unilyrics-");
            directory.toFile().deleteOnExit();
            for (String name : new String[]{"libunilyrics_ffi.so", "libunilyrics_jni.so"}) {
                try (java.io.InputStream resource = UniLyrics.class.getResourceAsStream(prefix + name)) {
                    if (resource == null) throw new IllegalStateException("Missing bundled native library: " + name + "; use a platform SDK jar or load(path)");
                    java.nio.file.Path file = directory.resolve(name);
                    java.nio.file.Files.copy(resource, file);
                    file.toFile().deleteOnExit();
                    System.load(file.toAbsolutePath().toString());
                }
            }
            bundledLoaded = true;
        } catch (java.io.IOException error) {
            throw new java.io.UncheckedIOException("Cannot extract UniLyrics native libraries", error);
        }
    }

    /** Load the JNI bridge by absolute path; it links to the shared Rust library. */
    public static void load(String libraryPath) {
        System.load(java.nio.file.Paths.get(libraryPath).toAbsolutePath().toString());
    }

    /** Load app/system-provided native libraries, including Android jniLibs. */
    public static void loadSystem() {
        System.loadLibrary("unilyrics_ffi");
        System.loadLibrary("unilyrics_jni");
    }

    private static native byte[] processBytes(byte[] request);

    /** Return the full JSON response, including structured core errors. */
    public static String process(String request) {
        if (request == null) throw new NullPointerException("request");
        byte[] input;
        try {
            java.nio.ByteBuffer bytes = StandardCharsets.UTF_8.newEncoder()
                .onMalformedInput(java.nio.charset.CodingErrorAction.REPORT)
                .onUnmappableCharacter(java.nio.charset.CodingErrorAction.REPORT)
                .encode(java.nio.CharBuffer.wrap(request));
            input = new byte[bytes.remaining()];
            bytes.get(input);
        } catch (java.nio.charset.CharacterCodingException error) {
            throw new IllegalArgumentException("Request contains invalid Unicode", error);
        }
        if (input.length > 1048576) throw new IllegalArgumentException("Request exceeds 1 MiB");
        byte[] output = processBytes(input);
        if (output == null) throw new IllegalStateException("UniLyrics returned no response");
        return new String(output, StandardCharsets.UTF_8);
    }

    public static final class LyricsException extends RuntimeException {
        private final String code;
        public LyricsException(String code, String message) { super(message); this.code = code; }
        public String getCode() { return code; }
    }

    public static Object call(Map<String, Object> request) {
        var response = Json.decode(process(Json.encode(request)));
        if (!Boolean.TRUE.equals(response.get("ok"))) {
            var error = objectResult(response.get("error"));
            throw new LyricsException((String)error.get("code"), (String)error.get("message"));
        }
        return response.get("result");
    }

    @SuppressWarnings("unchecked")
    static Map<String, Object> objectResult(Object result) {
        if (!(result instanceof Map)) throw new IllegalStateException("Expected an object result from UniLyrics");
        return (Map<String, Object>)result;
    }

    public static Map<String, Object> parse(String input) { return parse(input, null); }
    public static Map<String, Object> parse(String input, String format) {
        Map<String, Object> request = new LinkedHashMap<>();
        request.put("op", "parse"); request.put("input", java.util.Objects.requireNonNull(input));
        if (format != null) request.put("format", format);
        return objectResult(call(request));
    }

    public static String detect(String input) {
        return (String)call(Json.request("op", "detect", "input", java.util.Objects.requireNonNull(input)));
    }

    public static Map<String, Object> export(Map<String, Object> document, String format) {
        return export(document, format, "compatible");
    }
    public static Map<String, Object> export(Map<String, Object> document, String format, String policy) {
        return objectResult(call(Json.request("op", "export", "document", document, "format", format, "policy", policy)));
    }

    public static void validate(Map<String, Object> document) {
        call(Json.request("op", "validate", "document", document));
    }
    public static Map<String, Object> inspect(Map<String, Object> document) {
        return objectResult(call(Json.request("op", "inspect", "document", document)));
    }

    public static LyricsModel.ParseResult parseTyped(String input) { return parseTyped(input, null); }
    public static LyricsModel.ParseResult parseTyped(String input, LyricsModel.Format format) {
        return LyricsModel.ParseResult.fromMap(parse(input, format == null ? null : format.wireName()));
    }

    public static LyricsModel.Format detectTyped(String input) {
        return LyricsModel.Format.fromWireName(detect(input));
    }

    public static LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Format format) {
        return export(document, format, LyricsModel.Policy.COMPATIBLE);
    }
    public static LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Format format, LyricsModel.Policy policy) {
        return LyricsModel.ExportResult.fromMap(export(document.toMap(), format.wireName(), policy.wireName()));
    }

    public static void validate(LyricsModel.Document document) { validate(document.toMap()); }
    public static LyricsModel.Features inspect(LyricsModel.Document document) {
        return LyricsModel.Features.fromMap(inspect(document.toMap()));
    }

    public static Map<String, Object> normalize(Map<String, Object> document, long offsetUs) {
        return objectResult(call(Json.request("op", "normalize", "document", document, "offset_us", offsetUs)));
    }
    public static LyricsModel.ParseResult normalize(LyricsModel.Document document, long offsetUs) {
        return LyricsModel.ParseResult.fromMap(normalize(document.toMap(), offsetUs));
    }

    public static Map<String, Object> project(Map<String, Object> document) { return project(document, Json.request()); }
    public static Map<String, Object> project(Map<String, Object> document, Map<String, Object> options) {
        return objectResult(call(Json.request("op", "project", "document", document, "options", options)));
    }
    public static LyricsModel.ParseResult project(LyricsModel.Document document) { return project(document, new LyricsModel.ProjectionOptions()); }
    public static LyricsModel.ParseResult project(LyricsModel.Document document, LyricsModel.ProjectionOptions options) {
        return LyricsModel.ParseResult.fromMap(project(document.toMap(), options.toMap()));
    }
    public static Map<String, Object> exportProfile(Map<String, Object> document, String format) {
        return exportProfile(document, format, Json.request(), "compatible");
    }
    public static Map<String, Object> exportProfile(Map<String, Object> document, String format, Map<String, Object> options) {
        return exportProfile(document, format, options, "compatible");
    }
    public static Map<String, Object> exportProfile(Map<String, Object> document, String format, Map<String, Object> options, String policy) {
        return objectResult(call(Json.request("op", "export_profile", "document", document, "format", format, "options", options, "policy", policy)));
    }
    public static LyricsModel.ExportResult exportProfile(LyricsModel.Document document, LyricsModel.Format format) {
        return exportProfile(document, format, new LyricsModel.ProjectionOptions());
    }
    public static LyricsModel.ExportResult exportProfile(LyricsModel.Document document, LyricsModel.Format format, LyricsModel.ProjectionOptions options) {
        return exportProfile(document, format, options, LyricsModel.Policy.COMPATIBLE);
    }
    public static LyricsModel.ExportResult exportProfile(LyricsModel.Document document, LyricsModel.Format format, LyricsModel.ProjectionOptions options, LyricsModel.Policy policy) {
        return LyricsModel.ExportResult.fromMap(exportProfile(document.toMap(), format.wireName(), options.toMap(), policy.wireName()));
    }

    public static Map<String, Object> exportPreserved(Map<String, Object> document, String format) {
        return exportPreserved(document, format, Json.request());
    }
    public static Map<String, Object> exportPreserved(Map<String, Object> document, String format, Map<String, Object> options) {
        var result = objectResult(call(Json.request("op", "export_preserved", "document", document, "format", format, "options", options)));
        var restore = Json.request("op", "restore_preserved", "input", result.get("output"), "format", format, "sidecar", result.get("sidecar"));
        if (Json.encode(restore).getBytes(StandardCharsets.UTF_8).length > 1048576) {
            throw new LyricsException("INPUT_TOO_LARGE", "Encoded preservation restore request exceeds 1 MiB");
        }
        return result;
    }
    public static LyricsModel.PreservedExportResult exportPreserved(LyricsModel.Document document, LyricsModel.Format format) {
        return exportPreserved(document, format, new LyricsModel.ProjectionOptions());
    }
    public static LyricsModel.PreservedExportResult exportPreserved(LyricsModel.Document document, LyricsModel.Format format, LyricsModel.ProjectionOptions options) {
        return LyricsModel.PreservedExportResult.fromMap(exportPreserved(document.toMap(), format.wireName(), options.toMap()));
    }
    public static Map<String, Object> restorePreserved(String input, String format, Map<String, Object> sidecar) {
        return objectResult(call(Json.request("op", "restore_preserved", "input", input, "format", format, "sidecar", sidecar)));
    }
    public static LyricsModel.ParseResult restorePreserved(String input, LyricsModel.Format format, LyricsModel.PreservationSidecar sidecar) {
        return LyricsModel.ParseResult.fromMap(restorePreserved(input, format.wireName(), sidecar.toMap()));
    }

}
