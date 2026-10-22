package org.tharukrj.unilyrics.example

import org.tharukrj.unilyrics.Lyrics
import org.tharukrj.unilyrics.LyricsModel

object KotlinExample {
    @JvmStatic
    fun run(): String {
        Lyrics.loadSystem()
        val parsed = Lyrics.parseTyped("[00:01.25]Hello 世界 🎵", LyricsModel.Format.LRC)
        val converted = Lyrics.export(parsed.document(), LyricsModel.Format.PLAIN, LyricsModel.Policy.COMPATIBLE)
        check(converted.losses().isNotEmpty()) { "Plain export must report timing loss" }
        return converted.output()
    }
}
