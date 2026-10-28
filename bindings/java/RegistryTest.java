import org.tharukrj.unilyrics.LyricsModel;
import org.tharukrj.unilyrics.Registry;
import org.tharukrj.unilyrics.UniLyrics;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import org.json.JSONArray;

public class RegistryTest {
    static final Registry.Adapter WRAPPED = new Registry.Adapter() {
        public boolean matches(String input) { return input.startsWith("CUSTOM:"); }
        public LyricsModel.ParseResult parse(String input) {
            if (!matches(input)) throw new UniLyrics.LyricsException("BAD_HEADER", "Missing CUSTOM prefix");
            return UniLyrics.parseTyped(input.substring("CUSTOM:".length()), LyricsModel.Format.JSON);
        }
        public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) {
            var result = UniLyrics.export(document, LyricsModel.Format.JSON, policy);
            return new LyricsModel.ExportResult("CUSTOM:" + result.output(), result.losses());
        }
    };

    static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
    static void expect(String code, Runnable action) {
        try { action.run(); throw new AssertionError("Expected " + code); }
        catch (UniLyrics.LyricsException error) { check(error.getCode().equals(code), error.toString()); }
    }

    public static void main(String[] args) throws Exception {
        UniLyrics.load(args[0]);
        var registry = new Registry();
        registry.register("x-wrapped", WRAPPED);
        var cases = new JSONArray(Files.readString(Path.of(args[1])));
        for (int i = 0; i < cases.length(); i++) {
            var test = cases.getJSONObject(i);
            if (test.has("error")) continue;
            var format = LyricsModel.Format.fromWireName(test.getString("format"));
            String input = test.has("input") ? test.getString("input") : test.getJSONObject("document").toString();
            var direct = UniLyrics.parseTyped(input, format);
            check(registry.parse(input, format.wireName()).equals(direct), "builtin dispatch " + test.getString("name"));
            String before = UniLyrics.export(direct.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT).output();
            var encoded = registry.export(direct.document(), "x-wrapped", LyricsModel.Policy.STRICT);
            check(encoded.losses().isEmpty(), "unexpected loss");
            check(registry.detect(encoded.output()).equals("x-wrapped"), "custom detection");
            check(registry.parse(encoded.output()).document().equals(direct.document()), "custom round trip " + test.getString("name"));
            check(UniLyrics.export(direct.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT).output().equals(before), "input changed");
        }
        for (String name : new String[]{"plain", "x-", "X-custom", "x-💻", "x-ok\n", "x-" + "a".repeat(63)}) {
            expect("INVALID_CUSTOM_FORMAT", () -> registry.register(name, WRAPPED));
        }
        expect("DUPLICATE_FORMAT", () -> registry.register("x-wrapped", WRAPPED));
        expect("UNKNOWN_FORMAT", () -> new Registry().parse("CUSTOM:{}", "x-wrapped"));
        expect("UNKNOWN_FORMAT", () -> registry.parse("hi", "not-registered"));
        registry.register("x-two", WRAPPED);
        expect("AMBIGUOUS_FORMAT", () -> registry.detect("CUSTOM:{}"));
        var broad = new Registry();
        broad.register("x-broad", new Registry.Adapter() { public boolean matches(String input) { return true; } });
        expect("AMBIGUOUS_FORMAT", () -> broad.detect("[00:01]hi"));
        check(broad.parse("[00:01]hi", "lrc").document().cues().get(0).startUs() == 1_000_000L, "explicit builtin");
        expect("UNSUPPORTED_OPERATION", () -> broad.parse("hi", "x-broad"));
        try { broad.parse("\uD800", "x-broad"); throw new AssertionError("invalid Unicode accepted"); }
        catch (IllegalArgumentException expected) { }
        expect("INPUT_TOO_LARGE", () -> broad.parse("x".repeat(1048577), "x-broad"));
        broad.register("x-invalid", new Registry.Adapter() {
            public LyricsModel.ParseResult parse(String input) {
                return new LyricsModel.ParseResult(new LyricsModel.Document(999, List.of(), List.of(), Map.of(), null, List.of(), List.of()), List.of());
            }
        });
        expect("UNSUPPORTED_VERSION", () -> broad.parse("hi", "x-invalid"));
        testLossesAndSnapshots();
        testConcurrency();
        System.out.println("Java registry rich-corpus, preservation, concurrency and reentrancy checks passed.");
    }

    static void testLossesAndSnapshots() {
        var original = UniLyrics.parseTyped("සිංහල 👩🏽‍💻", LyricsModel.Format.PLAIN).document();
        original.attributes().put("nested", new java.util.LinkedHashMap<>(Map.of("value", 1)));
        String before = UniLyrics.export(original, LyricsModel.Format.JSON, LyricsModel.Policy.STRICT).output();
        var registry = new Registry();
        registry.register("x-mutate", new Registry.Adapter() {
            public LyricsModel.ParseResult parse(String input) { return WRAPPED.parse(input); }
            @SuppressWarnings("unchecked")
            public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) {
                document.cues().get(0).segments().set(0, new LyricsModel.Segment("changed"));
                ((Map<String, Object>)document.attributes().get("nested")).put("value", 2);
                return WRAPPED.export(document, policy);
            }
        });
        check(registry.export(original, "x-mutate").losses().get(0).code().equals("CUSTOM_ROUNDTRIP_CHANGED"), "silent mutation");
        expect("LOSSY_CONVERSION", () -> registry.export(original, "x-mutate", LyricsModel.Policy.STRICT));
        check(UniLyrics.export(original, LyricsModel.Format.JSON, LyricsModel.Policy.STRICT).output().equals(before), "nested original changed");
        registry.register("x-output", new Registry.Adapter() {
            public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) { return new LyricsModel.ExportResult("opaque", List.of()); }
        });
        check(registry.export(original, "x-output").losses().get(0).code().equals("CUSTOM_ROUNDTRIP_UNVERIFIED"), "unverified output");
        expect("LOSSY_CONVERSION", () -> registry.export(original, "x-output", LyricsModel.Policy.STRICT));
        registry.register("x-large", new Registry.Adapter() {
            public LyricsModel.ParseResult parse(String input) { throw new AssertionError("oversized output must not be reparsed"); }
            public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) { return new LyricsModel.ExportResult("x".repeat(1048577), List.of()); }
        });
        expect("INPUT_TOO_LARGE", () -> registry.export(original, "x-large"));
        var reported = new ArrayList<>(List.of(new LyricsModel.Diagnostic("ORIGINAL_LOSS", "reported", null)));
        registry.register("x-snapshot", new Registry.Adapter() {
            public LyricsModel.ParseResult parse(String input) { reported.set(0, new LyricsModel.Diagnostic("MUTATED_LOSS", "changed", null)); return WRAPPED.parse(input); }
            public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) { return new LyricsModel.ExportResult(WRAPPED.export(document, policy).output(), reported); }
        });
        check(registry.export(original, "x-snapshot").losses().get(0).code().equals("ORIGINAL_LOSS"), "result alias escaped");
        expect("LOSSY_CONVERSION", () -> registry.export(original, "x-snapshot", LyricsModel.Policy.STRICT));
        var sentinel = new IllegalStateException("application failure");
        registry.register("x-error", new Registry.Adapter() {
            public LyricsModel.ParseResult parse(String input) { throw sentinel; }
            public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) { return new LyricsModel.ExportResult("opaque", List.of()); }
        });
        try { registry.parse("hi", "x-error"); throw new AssertionError("missing callback failure"); }
        catch (IllegalStateException error) { check(error == sentinel, "callback error replaced"); }
        check(registry.export(original, "x-error").losses().get(0).code().equals("CUSTOM_ROUNDTRIP_UNVERIFIED"), "reparse exception");
    }

    static void testConcurrency() throws Exception {
        var registry = new Registry();
        registry.register("x-wrapped", WRAPPED);
        var original = UniLyrics.parseTyped("hi", LyricsModel.Format.PLAIN).document();
        String input = WRAPPED.export(original, LyricsModel.Policy.STRICT).output();
        var executor = Executors.newFixedThreadPool(4);
        try {
            var futures = new ArrayList<java.util.concurrent.Future<?>>();
            for (int i = 0; i < 8; i++) {
                final int index = i;
                futures.add(executor.submit(() -> {
                    registry.register("x-extra-" + index, new Registry.Adapter() {});
                    for (int j = 0; j < 10; j++) {
                        check(registry.parse(input).document().equals(original), "concurrent parse");
                        check(registry.export(original, "x-wrapped", LyricsModel.Policy.STRICT).losses().isEmpty(), "concurrent export");
                    }
                }));
            }
            for (var future : futures) future.get(20, TimeUnit.SECONDS);
        } finally { executor.shutdownNow(); }
        registry.register("x-reentrant", new Registry.Adapter() {
            public LyricsModel.ParseResult parse(String input) {
                registry.register("x-inside-callback", new Registry.Adapter() {});
                return new LyricsModel.ParseResult(original, List.of());
            }
        });
        check(registry.parse("hi", "x-reentrant").document().equals(original), "reentrant registration");
    }
}
