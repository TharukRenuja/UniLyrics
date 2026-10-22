package org.tharukrj.unilyrics.example;

import org.tharukrj.unilyrics.LyricsModel;
import org.tharukrj.unilyrics.UniLyrics;

public final class JavaExample {
    private JavaExample() {}

    public static String run() {
        UniLyrics.loadSystem();
        var parsed = UniLyrics.parseTyped("[00:01.25]Hello 世界 🎵", LyricsModel.Format.LRC);
        var converted = UniLyrics.export(parsed.document(), LyricsModel.Format.PLAIN, LyricsModel.Policy.COMPATIBLE);
        if (converted.losses().isEmpty()) throw new AssertionError("Plain export must report timing loss");
        return converted.output();
    }
}
