package org.tharukrj.unilyrics;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class JsonTest {
    public static void main(String[] args) {
        Map<String, Object> input = new LinkedHashMap<>();
        input.put("unicode", "你好 🎵 / \\\"");
        input.put("null", null);
        input.put("nested", java.util.Arrays.asList(Json.request("array", java.util.Arrays.asList(true, 9007199254740991L))));
        if (!input.equals(Json.decode(Json.encode(input)))) throw new AssertionError("JSON round trip changed data");
        var decoded = Json.decode("{\"array\":[null,false,1.25],\"object\":{}}");
        if (!((List<?>)decoded.get("array")).contains(null)) throw new AssertionError("Null was lost");
        var document = new LyricsModel.Document(java.util.Arrays.asList(new LyricsModel.Cue("cue",
            java.util.Arrays.asList(new LyricsModel.Segment("你好 🎵")))));
        var restored = LyricsModel.Document.fromMap(Json.decode(Json.encode(document.toMap())));
        if (!document.equals(restored)) throw new AssertionError("Typed document changed data");
        var options = new LyricsModel.ProjectionOptions(LyricsModel.Profile.CUSTOM,
            java.util.Arrays.asList(LyricsModel.ProjectionFeature.LANGUAGES));
        if (!options.toMap().get("features").equals(java.util.Arrays.asList("languages"))) throw new AssertionError("Profile features changed");
        var cycle = new ArrayList<Object>();
        cycle.add(cycle);
        expectInvalid(() -> Json.encode(cycle));
        expectInvalid(() -> Json.encode(java.util.Collections.singletonMap(1, "bad key")));
        expectInvalid(() -> Json.encode(Json.request("bad", Double.NaN)));
        expectInvalid(() -> Json.decode("{"));
        expectInvalid(() -> Json.decode("{\"deep\":" + String.join("", java.util.Collections.nCopies(130, "[")) + "0" + String.join("", java.util.Collections.nCopies(130, "]")) + "}"));
        if (!Json.request("op", "parse").equals(Json.request("op", "parse"))) throw new AssertionError("Request fields");
        expectInvalid(() -> Json.request("op"));
        expectInvalid(() -> Json.request("op", "parse", "op", "detect"));
        try { Json.request("op", null); throw new AssertionError("Null request value accepted"); }
        catch (NullPointerException expected) { }
        var selected = new ArrayList<>(java.util.Arrays.asList(LyricsModel.ProjectionFeature.LANGUAGES));
        var copied = new LyricsModel.ProjectionOptions(LyricsModel.Profile.CUSTOM, selected);
        selected.clear();
        if (copied.features().size() != 1) throw new AssertionError("Features not copied");
        try { copied.features().clear(); throw new AssertionError("Features became mutable"); }
        catch (UnsupportedOperationException expected) { }
        try { ((List<?>)document.toMap().get("cues")).clear(); throw new AssertionError("Projected list became mutable"); }
        catch (UnsupportedOperationException expected) { }
        System.out.println("Shared Android/desktop JSON conversion checks passed.");
    }

    private static void expectInvalid(Runnable action) {
        try { action.run(); }
        catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("Invalid JSON value was accepted");
    }
}
