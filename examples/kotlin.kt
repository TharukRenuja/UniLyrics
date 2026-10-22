import org.tharukrj.unilyrics.Lyrics
import org.tharukrj.unilyrics.LyricsModel

fun main() {
    Lyrics.loadSystem()
    val parsed = Lyrics.parseTyped("[00:01]Hello 世界 🎵", LyricsModel.Format.LRC)
    val converted = Lyrics.export(parsed.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT)
    println(converted.output())
}
