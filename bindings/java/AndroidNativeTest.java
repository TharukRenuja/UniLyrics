package org.tharukrj.unilyrics;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;

public final class AndroidNativeTest {
    public static void main(String[] args) throws Exception {
        UniLyrics.loadSystem();
        UniLyrics.loadSystem();
        if (args.length > 0 && args[0].equals("--protocol")) {
            var input = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
            for (String line; (line = input.readLine()) != null;) System.out.println(UniLyrics.process(line));
            return;
        }
        var document = UniLyrics.parseTyped("[00:01]සිංහල 世界 🎵", LyricsModel.Format.LRC).document();
        if (document.cues().get(0).startUs() != 1000000L) throw new AssertionError("Timestamp");
        var shifted = UniLyrics.normalize(document, 500000).document();
        if (shifted.cues().get(0).startUs() != 1500000L) throw new AssertionError("Offset");
        var options = new LyricsModel.ProjectionOptions(LyricsModel.Profile.PLAIN);
        if (UniLyrics.project(document, options).document().cues().get(0).startUs() != null) throw new AssertionError("Profile");
        var bundle = UniLyrics.exportPreserved(document, LyricsModel.Format.PLAIN);
        var restored = UniLyrics.restorePreserved(bundle.output(), LyricsModel.Format.PLAIN, bundle.sidecar());
        if (!restored.document().equals(document)) throw new AssertionError("Preservation");
        var registry = new Registry();
        registry.register("x-demo", new Registry.Adapter() {
            public boolean matches(String input) { return input.startsWith("DEMO:"); }
            public LyricsModel.ParseResult parse(String input) { return UniLyrics.parseTyped(input.substring(5), LyricsModel.Format.PLAIN); }
        });
        if (!registry.parse("DEMO:世界", "x-demo").document().cues().get(0).segments().get(0).text().equals("世界")) throw new AssertionError("Registry");
        System.out.println("Android Java JNI typed helpers, profiles, preservation and registry passed.");
    }
}
