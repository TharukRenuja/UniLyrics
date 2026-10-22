package org.tharukrj.unilyrics

/** Kotlin/JVM facade over the same JNI bridge used by the Java SDK. */
object Lyrics {
    fun load() = UniLyrics.load()
    fun load(libraryPath: String) = UniLyrics.load(libraryPath)
    fun loadSystem() = UniLyrics.loadSystem()
    fun process(request: String): String = UniLyrics.process(request)
    fun parse(input: String, format: String? = null): Map<String, Any?> = UniLyrics.parse(input, format)
    fun detect(input: String): String = UniLyrics.detect(input)
    fun export(document: Map<String, Any?>, format: String, policy: String = "compatible"): Map<String, Any?> = UniLyrics.export(document, format, policy)
    fun inspect(document: Map<String, Any?>): Map<String, Any?> = UniLyrics.inspect(document)
    fun validate(document: Map<String, Any?>) = UniLyrics.validate(document)
    fun parseTyped(input: String, format: LyricsModel.Format? = null): LyricsModel.ParseResult = UniLyrics.parseTyped(input, format)
    fun detectTyped(input: String): LyricsModel.Format = UniLyrics.detectTyped(input)
    fun export(document: LyricsModel.Document, format: LyricsModel.Format, policy: LyricsModel.Policy = LyricsModel.Policy.COMPATIBLE): LyricsModel.ExportResult = UniLyrics.export(document, format, policy)
    fun inspect(document: LyricsModel.Document): LyricsModel.Features = UniLyrics.inspect(document)
    fun validate(document: LyricsModel.Document) = UniLyrics.validate(document)
    fun normalize(document: Map<String, Any?>, offsetUs: Long = 0): Map<String, Any?> = UniLyrics.normalize(document, offsetUs)
    fun normalize(document: LyricsModel.Document, offsetUs: Long = 0): LyricsModel.ParseResult = UniLyrics.normalize(document, offsetUs)
    fun project(document: Map<String, Any?>, options: Map<String, Any?> = emptyMap()): Map<String, Any?> = UniLyrics.project(document, options)
    fun project(document: LyricsModel.Document, options: LyricsModel.ProjectionOptions = LyricsModel.ProjectionOptions()): LyricsModel.ParseResult = UniLyrics.project(document, options)
    fun exportProfile(document: Map<String, Any?>, format: String, options: Map<String, Any?> = emptyMap(), policy: String = "compatible"): Map<String, Any?> = UniLyrics.exportProfile(document, format, options, policy)
    fun exportProfile(document: LyricsModel.Document, format: LyricsModel.Format, options: LyricsModel.ProjectionOptions = LyricsModel.ProjectionOptions(), policy: LyricsModel.Policy = LyricsModel.Policy.COMPATIBLE): LyricsModel.ExportResult = UniLyrics.exportProfile(document, format, options, policy)
    fun exportPreserved(document: Map<String, Any?>, format: String, options: Map<String, Any?> = emptyMap()): Map<String, Any?> = UniLyrics.exportPreserved(document, format, options)
    fun exportPreserved(document: LyricsModel.Document, format: LyricsModel.Format, options: LyricsModel.ProjectionOptions = LyricsModel.ProjectionOptions()): LyricsModel.PreservedExportResult = UniLyrics.exportPreserved(document, format, options)
    fun restorePreserved(input: String, format: String, sidecar: Map<String, Any?>): Map<String, Any?> = UniLyrics.restorePreserved(input, format, sidecar)
    fun restorePreserved(input: String, format: LyricsModel.Format, sidecar: LyricsModel.PreservationSidecar): LyricsModel.ParseResult = UniLyrics.restorePreserved(input, format, sidecar)
}
