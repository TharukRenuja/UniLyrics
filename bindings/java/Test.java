import org.tharukrj.unilyrics.UniLyrics;
import org.tharukrj.unilyrics.LyricsModel;
import java.util.List;
import java.util.Map;
import org.json.JSONObject;
public class Test {
    static void profileChecks() {
        var document = UniLyrics.parseTyped("[00:01]hello", LyricsModel.Format.LRC).document();
        if (!UniLyrics.project(document).document().equals(document)) throw new AssertionError("profile default");
        var options = new LyricsModel.ProjectionOptions(LyricsModel.Profile.CUSTOM, List.of());
        var projected = UniLyrics.project(document, options);
        if (projected.document().cues().get(0).startUs() != null || projected.diagnostics().isEmpty()) throw new AssertionError("empty custom selection");
        var exported = UniLyrics.exportProfile(document, LyricsModel.Format.JSON, options);
        if (!UniLyrics.parseTyped(exported.output(), LyricsModel.Format.JSON).document().equals(projected.document()) || !exported.losses().equals(projected.diagnostics())) throw new AssertionError("profile export");
        for (var invalid : List.of(new LyricsModel.ProjectionOptions(LyricsModel.Profile.CUSTOM),
            new LyricsModel.ProjectionOptions(LyricsModel.Profile.FULL, List.of()),
            new LyricsModel.ProjectionOptions(LyricsModel.Profile.CUSTOM, List.of(LyricsModel.ProjectionFeature.AGENTS, LyricsModel.ProjectionFeature.AGENTS)))) {
            try { UniLyrics.project(document, invalid); throw new AssertionError("invalid selection accepted"); }
            catch (UniLyrics.LyricsException expected) { }
        }
        try { UniLyrics.exportProfile(document, LyricsModel.Format.JSON, options, LyricsModel.Policy.STRICT); throw new AssertionError("strict accepted loss"); }
        catch (UniLyrics.LyricsException expected) { if (!expected.getCode().equals("LOSSY_CONVERSION")) throw expected; }
        if (document.cues().get(0).startUs() != 1_000_000L) throw new AssertionError("input mutated");
    }

    static void preservationChecks() {
        String fixture;
        try { fixture = java.nio.file.Files.readString(java.nio.file.Path.of("crates/unilyrics/tests/fixtures/profiles.json")); }
        catch (java.io.IOException error) { throw new java.io.UncheckedIOException(error); }
        for (Object item : new org.json.JSONArray(fixture)) {
            var test = (JSONObject)item;
            var request = test.getJSONObject("request");
            if (test.has("error") || !request.getString("op").equals("project")) continue;
            var document = UniLyrics.parseTyped(request.getJSONObject("document").toString(), LyricsModel.Format.JSON).document();
            var options = request.optJSONObject("options");
            var profile = options == null ? LyricsModel.Profile.FULL : LyricsModel.Profile.valueOf(options.optString("profile", "full").toUpperCase(java.util.Locale.ROOT));
            List<LyricsModel.ProjectionFeature> features = options == null || !options.has("features") ? null : options.getJSONArray("features").toList().stream().map(v -> LyricsModel.ProjectionFeature.valueOf(((String)v).toUpperCase(java.util.Locale.ROOT))).toList();
            var bundle = UniLyrics.exportPreserved(document, LyricsModel.Format.JSON, new LyricsModel.ProjectionOptions(profile, features));
            if (!new JSONObject(bundle.output()).similar(test.getJSONObject("expected_document"))) throw new AssertionError(test.getString("name"));
            if (!bundle.losses().stream().map(LyricsModel.Diagnostic::code).toList().equals(test.getJSONArray("codes").toList())) throw new AssertionError("losses");
            var reloaded = LyricsModel.PreservationSidecar.fromMap(new JSONObject(json(bundle.sidecar().toMap()).toString()).toMap());
            if (!UniLyrics.restorePreserved(bundle.output(), LyricsModel.Format.JSON, reloaded).document().equals(document)) throw new AssertionError("snapshot restore");
            if (!((JSONObject)json(UniLyrics.restorePreserved(bundle.output(), "json", reloaded.toMap()).get("document"))).similar((JSONObject)json(document.toMap()))) throw new AssertionError("map restore");
            var raw = UniLyrics.exportPreserved(document.toMap(), "json", options == null ? Map.of() : options.toMap());
            if (!LyricsModel.PreservedExportResult.fromMap(raw).equals(bundle)) throw new AssertionError("map export");
            try { UniLyrics.restorePreserved(bundle.output()+"\n", LyricsModel.Format.JSON, reloaded); throw new AssertionError("stale sidecar"); }
            catch (UniLyrics.LyricsException expected) { if (!expected.getCode().equals("SIDECAR_MISMATCH")) throw expected; }
            try { UniLyrics.restorePreserved(bundle.output(), LyricsModel.Format.PLAIN, reloaded); throw new AssertionError("wrong format"); }
            catch (UniLyrics.LyricsException expected) { if (!expected.getCode().equals("SIDECAR_MISMATCH")) throw expected; }
            var invalid = new LyricsModel.PreservationSidecar(2, reloaded.format(), reloaded.output(), reloaded.document());
            try { UniLyrics.restorePreserved(bundle.output(), LyricsModel.Format.JSON, invalid); throw new AssertionError("unknown version"); }
            catch (UniLyrics.LyricsException expected) { if (!expected.getCode().equals("UNSUPPORTED_SIDECAR_VERSION")) throw expected; }
        }
        var document = UniLyrics.parseTyped("[00:01]සිංහල 👩🏽‍💻", LyricsModel.Format.LRC).document();
        var bundle = UniLyrics.exportPreserved(document, LyricsModel.Format.PLAIN);
        var invalidMap = new java.util.LinkedHashMap<>(bundle.sidecar().toMap());
        invalidMap.put("version", 1.5);
        try { LyricsModel.PreservationSidecar.fromMap(invalidMap); throw new AssertionError("fractional version normalized"); }
        catch (UniLyrics.LyricsException expected) { if (!expected.getCode().equals("INVALID_REQUEST")) throw expected; }
        invalidMap = new java.util.LinkedHashMap<>(bundle.sidecar().toMap());
        invalidMap.put("unexpected", true);
        try { LyricsModel.PreservationSidecar.fromMap(invalidMap); throw new AssertionError("unknown sidecar field dropped"); }
        catch (UniLyrics.LyricsException expected) { if (!expected.getCode().equals("INVALID_REQUEST")) throw expected; }
        if (bundle.losses().isEmpty() || !UniLyrics.restorePreserved(bundle.output(), LyricsModel.Format.PLAIN, bundle.sidecar()).document().equals(document)) throw new AssertionError("plain preservation");
        var large = UniLyrics.parseTyped("</".repeat(120_000), LyricsModel.Format.PLAIN).document();
        try { UniLyrics.exportPreserved(large, LyricsModel.Format.PLAIN); throw new AssertionError("unrestorable encoded bundle returned"); }
        catch (UniLyrics.LyricsException expected) { if (!expected.getCode().equals("INPUT_TOO_LARGE")) throw expected; }
        var smaller = UniLyrics.parseTyped("</".repeat(40_000), LyricsModel.Format.PLAIN).document();
        var validBundle = UniLyrics.exportPreserved(smaller, LyricsModel.Format.PLAIN);
        if (!UniLyrics.restorePreserved(validBundle.output(), LyricsModel.Format.PLAIN, validBundle.sidecar()).document().equals(smaller)) throw new AssertionError("smaller escaped bundle");
    }

    public static void main(String[] args) {
        if (args[0].equals("--system")) { UniLyrics.loadSystem(); UniLyrics.loadSystem(); }
        else UniLyrics.load(args[0]);
        if (args.length > 1 && args[1].equals("--typed-protocol")) {
            try (var input = new java.util.Scanner(System.in, java.nio.charset.StandardCharsets.UTF_8)) {
                while (input.hasNextLine()) System.out.println(typedResponse(input.nextLine()));
            }
            return;
        }
        if (args.length > 1 && args[1].equals("--protocol")) {
            try (java.util.Scanner input = new java.util.Scanner(System.in, java.nio.charset.StandardCharsets.UTF_8)) {
                while (input.hasNextLine()) System.out.println(UniLyrics.process(input.nextLine()));
            }
            return;
        }
        for (int i=0; i<1000; i++) {
            String result = UniLyrics.process("{\"op\":\"parse\",\"input\":\"[00:01.25]你好 👩🏽‍💻\",\"format\":\"lrc\"}");
            if (!result.contains("1250000") || !result.contains("你好 👩🏽‍💻")) throw new AssertionError(result);
        }
        if (!UniLyrics.process("{broken").contains("INVALID_REQUEST")) throw new AssertionError("error propagation");
        try {
            UniLyrics.process("\uD800");
            throw new AssertionError("Invalid Unicode must not be replaced");
        } catch (IllegalArgumentException expected) { }
        if (!UniLyrics.detect("[00:01]hi").equals("lrc")) throw new AssertionError("detect");
        var parsed = UniLyrics.parse("hello", "plain");
        @SuppressWarnings("unchecked")
        var document = (java.util.Map<String,Object>) parsed.get("document");
        UniLyrics.validate(document);
        if (!UniLyrics.inspect(document).get("line_timing").equals(false)) throw new AssertionError("inspect");
        if (!UniLyrics.export(document, "plain").get("output").equals("hello")) throw new AssertionError("export");
        try {
            UniLyrics.parse("[00:60]bad", "lrc");
            throw new AssertionError("Missing structured error");
        } catch (UniLyrics.LyricsException expected) {
            if (!expected.getCode().equals("INVALID_TIMESTAMP")) throw new AssertionError(expected);
        }
        profileChecks();
        preservationChecks();
        var typed = UniLyrics.parseTyped("[00:01.25]සිංහල 👩🏽‍💻", LyricsModel.Format.LRC);
        if (typed.document().cues().get(0).startUs() != 1_250_000L) throw new AssertionError("typed time");
        UniLyrics.validate(typed.document());
        var shifted = UniLyrics.normalize(typed.document(), -250_000L);
        if (shifted.document().cues().get(0).startUs() != 1_000_000L || typed.document().cues().get(0).startUs() != 1_250_000L) throw new AssertionError("typed normalization");
        if (!UniLyrics.inspect(typed.document()).lineTiming()) throw new AssertionError("typed inspect");
        if (UniLyrics.detectTyped("[00:01]hi") != LyricsModel.Format.LRC) throw new AssertionError("typed detect");
        var typedJSON = UniLyrics.export(typed.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT);
        if (!UniLyrics.parseTyped(typedJSON.output(), LyricsModel.Format.JSON).document().equals(typed.document())) throw new AssertionError("typed model round trip");
        Map<String, Object> attributes = new java.util.LinkedHashMap<>();
        attributes.put("precise", 9_007_199_254_740_993L);
        attributes.put("unsigned", new java.math.BigInteger("18446744073709551615"));
        attributes.put("null", null);
        attributes.put("nested", List.of(Map.of("text", "සිංහල 👩🏽‍💻")));
        var authored = new LyricsModel.Document(1,
            List.of(new LyricsModel.Cue("cue", 0L, 1_000_000L,
                List.of(new LyricsModel.Segment("Hello", 0L, 500_000L, Map.of(), "word", "en", List.of("singer"), List.of("lead"))),
                Map.of(), "en", List.of(), List.of(), null)),
            List.of(new LyricsModel.Metadata("title", "Song")), attributes, "en",
            List.of(new LyricsModel.Agent("singer", "Voice", "person", Map.of())),
            List.of(new LyricsModel.Auxiliary("translation", "translation", "si", List.of("word"), List.of(new LyricsModel.Segment("ආයුබෝවන්")), Map.of())));
        UniLyrics.validate(authored);
        var encoded = UniLyrics.export(authored, LyricsModel.Format.JSON, LyricsModel.Policy.STRICT);
        var again = UniLyrics.parseTyped(encoded.output(), LyricsModel.Format.JSON).document();
        if (!again.equals(authored)) throw new AssertionError("authored rich/null/integer round trip");
        if (!UniLyrics.inspect(authored).auxiliaryKinds().equals(List.of("translation"))) throw new AssertionError("auxiliary features");
        try { UniLyrics.export(authored, LyricsModel.Format.PLAIN, LyricsModel.Policy.STRICT); throw new AssertionError("missing typed loss"); }
        catch (UniLyrics.LyricsException error) { if (!error.getCode().equals("LOSSY_CONVERSION")) throw error; }
        var invalid = new LyricsModel.Document(List.of(new LyricsModel.Cue("cue", -1L, null, List.of(), Map.of(), null, List.of(), List.of(), null)));
        try { UniLyrics.validate(invalid); throw new AssertionError("negative typed timestamp accepted"); }
        catch (UniLyrics.LyricsException expected) { }
        System.out.println("Java/JNI Unicode and repeated-call checks passed.");
    }

    // JSONObject(Map) drops null-valued entries; fixtures require explicit null times.
    static Object json(Object value) {
        if (value == null) return JSONObject.NULL;
        if (value instanceof Map<?, ?> map) {
            var object = new JSONObject();
            for (var entry : map.entrySet()) object.put((String)entry.getKey(), json(entry.getValue()));
            return object;
        }
        if (value instanceof List<?> list) {
            var array = new org.json.JSONArray();
            for (var item : list) array.put(json(item));
            return array;
        }
        return value;
    }

    static String typedResponse(String line) {
        try {
            var request = new JSONObject(line);
            var op = request.getString("op");
            if (op.equals("export_preserved") || op.equals("restore_preserved")) return UniLyrics.process(line);
            Object result;
            if (op.equals("parse")) {
                var format = request.isNull("format") ? null : LyricsModel.Format.fromWireName(request.getString("format"));
                result = UniLyrics.parseTyped(request.getString("input"), format).toMap();
            } else {
                if (op.equals("project") || op.equals("export_profile")) {
                    var rawDocument = request.getJSONObject("document").toMap();
                    var options = request.has("options") ? request.getJSONObject("options").toMap() : Map.<String, Object>of();
                    var projected = op.equals("project") ? UniLyrics.project(rawDocument, options)
                        : UniLyrics.exportProfile(rawDocument, request.getString("format"), options, request.optString("policy", "compatible"));
                    return json(Map.of("ok", true, "result", projected)).toString();
                }
                if (op.equals("normalize")) {
                    // Raw input preserves invalid-document test cases; exercise typed methods separately below.
                    return json(Map.of("ok", true, "result", UniLyrics.normalize(request.getJSONObject("document").toMap(), request.optLong("offset_us", 0)))).toString();
                }
                var document = UniLyrics.parseTyped(request.getJSONObject("document").toString(), LyricsModel.Format.JSON).document();
                result = switch (op) {
                    case "inspect" -> UniLyrics.inspect(document).toMap();
                    case "normalize" -> UniLyrics.normalize(document, request.optLong("offset_us", 0)).toMap();
                    case "validate" -> { UniLyrics.validate(document); yield Map.of("valid", true); }
                    case "export" -> UniLyrics.export(document, LyricsModel.Format.fromWireName(request.getString("format")),
                        request.optString("policy", "compatible").equals("strict") ? LyricsModel.Policy.STRICT : LyricsModel.Policy.COMPATIBLE).toMap();
                    default -> throw new AssertionError("Unsupported test operation: " + op);
                };
            }
            return json(Map.of("ok", true, "result", result)).toString();
        } catch (UniLyrics.LyricsException error) {
            return new JSONObject(Map.of("ok", false, "error", Map.of("code", error.getCode(), "message", error.getMessage()))).toString();
        }
    }
}
