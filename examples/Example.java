import org.tharukrj.unilyrics.LyricsModel;
import org.tharukrj.unilyrics.UniLyrics;

public final class Example {
    public static void main(String[] args) {
        UniLyrics.loadSystem();
        var parsed = UniLyrics.parseTyped("[00:01]Hello 世界 🎵", LyricsModel.Format.LRC);
        var converted = UniLyrics.export(parsed.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT);
        System.out.println(converted.output());
    }
}
