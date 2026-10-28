import org.tharukrj.unilyrics.Lyrics
import org.tharukrj.unilyrics.LyricsModel

fun main() {
    Lyrics.loadSystem()
    val document = Lyrics.parseTyped("[00:01]සිංහල 世界 🎵", LyricsModel.Format.LRC).document()
    check(document.cues()[0].startUs() == 1_000_000L)
    check(Lyrics.normalize(document, 500_000).document().cues()[0].startUs() == 1_500_000L)
    val options = LyricsModel.ProjectionOptions(LyricsModel.Profile.PLAIN)
    check(Lyrics.project(document, options).document().cues()[0].startUs() == null)
    val bundle = Lyrics.exportPreserved(document, LyricsModel.Format.PLAIN)
    check(Lyrics.restorePreserved(bundle.output(), LyricsModel.Format.PLAIN, bundle.sidecar()).document() == document)
    println("Android Kotlin JNI typed helpers, profiles and preservation passed.")
}
