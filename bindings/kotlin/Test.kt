import org.tharukrj.unilyrics.Lyrics
import org.tharukrj.unilyrics.LyricsModel
import org.tharukrj.unilyrics.Registry

fun main(args: Array<String>) {
    if (args[0] == "--system") { Lyrics.loadSystem(); Lyrics.loadSystem() }
    else Lyrics.load(args[0])
    if (args.size > 1 && args[1] == "--protocol") {
        System.`in`.bufferedReader(Charsets.UTF_8).useLines { lines ->
            lines.forEach { line ->
                val request = org.json.JSONObject(line)
                val op = request.getString("op")
                if (op == "project" || op == "export_profile") {
                    try {
                        val rawDocument = request.getJSONObject("document").toMap()
                        val options = request.optJSONObject("options")?.toMap() ?: emptyMap()
                        val result = if (op == "project") Lyrics.project(rawDocument, options)
                            else Lyrics.exportProfile(rawDocument, request.getString("format"), options, request.optString("policy", "compatible"))
                        println(Test.json(mapOf("ok" to true, "result" to result)))
                    } catch (error: org.tharukrj.unilyrics.UniLyrics.LyricsException) {
                        println(org.json.JSONObject(mapOf("ok" to false, "error" to mapOf("code" to error.code, "message" to error.message))))
                    }
                } else println(Lyrics.process(line))
            }
        }
        return
    }
    val response = Lyrics.process("""{"op":"parse","input":"[00:01.25]සිංහල","format":"lrc"}""")
    check(response.contains("1250000") && response.contains("සිංහල")) { response }
    check(Lyrics.process("{broken").contains("INVALID_REQUEST"))
    check(Lyrics.detect("[00:01]hi") == "lrc")
    @Suppress("UNCHECKED_CAST")
    val document = Lyrics.parse("hello", "plain")["document"] as Map<String, Any?>
    Lyrics.validate(document)
    check(Lyrics.inspect(document)["line_timing"] == false)
    check(Lyrics.export(document, "plain")["output"] == "hello")
    val profileDocument = Lyrics.parseTyped("[00:01]hello", LyricsModel.Format.LRC).document()
    check(Lyrics.project(profileDocument).document() == profileDocument)
    val profileOptions = LyricsModel.ProjectionOptions(LyricsModel.Profile.CUSTOM, emptyList())
    val projected = Lyrics.project(profileDocument, profileOptions)
    check(projected.document().cues()[0].startUs() == null && projected.diagnostics().isNotEmpty())
    val profileExport = Lyrics.exportProfile(profileDocument, LyricsModel.Format.JSON, profileOptions)
    check(Lyrics.parseTyped(profileExport.output(), LyricsModel.Format.JSON).document() == projected.document())
    check(profileExport.losses() == projected.diagnostics())
    check(Lyrics.project(document) == org.tharukrj.unilyrics.UniLyrics.project(document))
    check(Lyrics.exportProfile(document, "json")["output"] == Lyrics.export(document, "json")["output"])
    try {
        Lyrics.exportProfile(profileDocument, LyricsModel.Format.JSON, profileOptions, LyricsModel.Policy.STRICT)
        error("strict accepted loss")
    } catch (expected: org.tharukrj.unilyrics.UniLyrics.LyricsException) {
        check(expected.code == "LOSSY_CONVERSION")
    }
    val preservationCases = org.json.JSONArray(java.nio.file.Files.readString(java.nio.file.Path.of(args[1]).resolveSibling("profiles.json")))
    for (item in preservationCases) {
        val test = item as org.json.JSONObject
        val request = test.getJSONObject("request")
        if (test.has("error") || request.getString("op") != "project") continue
        val original = Lyrics.parseTyped(request.getJSONObject("document").toString(), LyricsModel.Format.JSON).document()
        val rawOptions = request.optJSONObject("options")
        val profile = LyricsModel.Profile.valueOf((rawOptions?.optString("profile", "full") ?: "full").uppercase(java.util.Locale.ROOT))
        val features = rawOptions?.optJSONArray("features")?.toList()?.map { LyricsModel.ProjectionFeature.valueOf((it as String).uppercase(java.util.Locale.ROOT)) }
        val bundle = Lyrics.exportPreserved(original, LyricsModel.Format.JSON, LyricsModel.ProjectionOptions(profile, features))
        check(org.json.JSONObject(bundle.output()).similar(test.getJSONObject("expected_document")))
        check(bundle.losses().map { it.code() } == test.getJSONArray("codes").toList())
        val reloaded = LyricsModel.PreservationSidecar.fromMap(org.json.JSONObject(Test.json(bundle.sidecar().toMap()).toString()).toMap())
        check(Lyrics.restorePreserved(bundle.output(), LyricsModel.Format.JSON, reloaded).document() == original)
        val rawBundle = Lyrics.exportPreserved(original.toMap(), "json", rawOptions?.toMap() ?: emptyMap())
        check(LyricsModel.PreservedExportResult.fromMap(rawBundle) == bundle)
        val rawRestored = Lyrics.restorePreserved(bundle.output(), "json", reloaded.toMap())
        check((Test.json(rawRestored["document"]) as org.json.JSONObject).similar(Test.json(original.toMap()) as org.json.JSONObject))
        try {
            Lyrics.restorePreserved(bundle.output() + "\n", LyricsModel.Format.JSON, reloaded)
            error("stale sidecar accepted")
        } catch (expected: org.tharukrj.unilyrics.UniLyrics.LyricsException) { check(expected.code == "SIDECAR_MISMATCH") }
        val invalid = LyricsModel.PreservationSidecar(2, reloaded.format(), reloaded.output(), reloaded.document())
        try {
            Lyrics.restorePreserved(bundle.output(), LyricsModel.Format.JSON, invalid)
            error("unknown sidecar version accepted")
        } catch (expected: org.tharukrj.unilyrics.UniLyrics.LyricsException) { check(expected.code == "UNSUPPORTED_SIDECAR_VERSION") }
    }
    val preservedPlain = Lyrics.exportPreserved(profileDocument, LyricsModel.Format.PLAIN)
    check(preservedPlain.losses().isNotEmpty())
    check(Lyrics.restorePreserved(preservedPlain.output(), LyricsModel.Format.PLAIN, preservedPlain.sidecar()).document() == profileDocument)
    val typed = Lyrics.parseTyped("[00:01.25]සිංහල 👩🏽‍💻", LyricsModel.Format.LRC)
    check(typed.document().cues()[0].startUs() == 1_250_000L)
    Lyrics.validate(typed.document())
    val shifted = Lyrics.normalize(typed.document(), -250_000L)
    check(shifted.document().cues()[0].startUs() == 1_000_000L)
    check(typed.document().cues()[0].startUs() == 1_250_000L)
    check(Lyrics.inspect(typed.document()).lineTiming())
    check(Lyrics.detectTyped("[00:01]hi") == LyricsModel.Format.LRC)
    val typedJSON = Lyrics.export(typed.document(), LyricsModel.Format.JSON, LyricsModel.Policy.STRICT)
    check(Lyrics.parseTyped(typedJSON.output(), LyricsModel.Format.JSON).document() == typed.document())
    val authored = LyricsModel.Document(listOf(LyricsModel.Cue("cue", listOf(LyricsModel.Segment("Hello")))))
    val plain = Lyrics.export(authored, LyricsModel.Format.PLAIN)
    check(plain.output() == "Hello")
    check(plain.losses().map { it.code() } == listOf("CUE_IDS_CHANGED"))
    try { Lyrics.parseTyped("[00:60]bad", LyricsModel.Format.LRC); error("missing typed error") }
    catch (failure: org.tharukrj.unilyrics.UniLyrics.LyricsException) { check(failure.code == "INVALID_TIMESTAMP") }
    val registry = Registry()
    registry.register("x-wrapped", object : Registry.Adapter {
        override fun matches(input: String) = input.startsWith("CUSTOM:")
        override fun parse(input: String): LyricsModel.ParseResult {
            check(matches(input))
            return Lyrics.parseTyped(input.removePrefix("CUSTOM:"), LyricsModel.Format.JSON)
        }
        override fun export(document: LyricsModel.Document, policy: LyricsModel.Policy): LyricsModel.ExportResult {
            val result = Lyrics.export(document, LyricsModel.Format.JSON, policy)
            return LyricsModel.ExportResult("CUSTOM:" + result.output(), result.losses())
        }
    })
    val cases = org.json.JSONArray(java.nio.file.Files.readString(java.nio.file.Path.of(args[1])))
    for (index in 0 until cases.length()) {
        val case = cases.getJSONObject(index)
        if (case.has("error")) continue
        val input = if (case.has("input")) case.getString("input") else case.getJSONObject("document").toString()
        val direct = Lyrics.parseTyped(input, LyricsModel.Format.fromWireName(case.getString("format")))
        check(registry.parse(input, case.getString("format")) == direct)
        val encoded = registry.export(direct.document(), "x-wrapped", LyricsModel.Policy.STRICT)
        check(encoded.losses().isEmpty())
        check(registry.detect(encoded.output()) == "x-wrapped")
        check(registry.parse(encoded.output()).document() == direct.document())
    }
    println("Kotlin/JVM native bridge checks passed.")
}
