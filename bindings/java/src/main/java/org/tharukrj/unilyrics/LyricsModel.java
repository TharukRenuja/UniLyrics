package org.tharukrj.unilyrics;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

/** Typed canonical values; parsing, validation and conversion stay in the shared core. */
public final class LyricsModel {
    private LyricsModel() {}

    public enum Format {
        PLAIN("plain"), LRC("lrc"), ENHANCED_LRC("enhanced_lrc"), JSON("json"),
        TTML("ttml"), TTML_LYRICS("ttml_lyrics"), SRT("srt"), WEBVTT("webvtt");
        private final String wireName;
        Format(String wireName) { this.wireName = wireName; }
        public String wireName() { return wireName; }
        public static Format fromWireName(String name) {
            for (Format format : values()) if (format.wireName.equals(name)) return format;
            throw new IllegalArgumentException("Unknown lyric format: " + name);
        }
    }

    public enum Policy {
        STRICT("strict"), COMPATIBLE("compatible");
        private final String wireName;
        Policy(String wireName) { this.wireName = wireName; }
        public String wireName() { return wireName; }
    }

    public enum Profile {
        FULL, LINE, WORD, KARAOKE, PLAIN, TRANSLATED, CUSTOM;
        public String wireName() { return name().toLowerCase(java.util.Locale.ROOT); }
    }

    public enum ProjectionFeature {
        LINE_TIMING, SEGMENT_TIMING, AGENTS, RELATIONSHIPS, LANGUAGES, ROLES, METADATA, ATTRIBUTES, AUXILIARY;
        public String wireName() { return name().toLowerCase(java.util.Locale.ROOT); }
    }

    /** Null features means omitted; an empty custom list selects text/structure only. */
    public record ProjectionOptions(Profile profile, List<ProjectionFeature> features) {
        public ProjectionOptions {
            java.util.Objects.requireNonNull(profile);
            if (features != null) {
                features = java.util.Collections.unmodifiableList(new ArrayList<>(features));
                features.forEach(java.util.Objects::requireNonNull);
            }
        }
        public ProjectionOptions() { this(Profile.FULL, null); }
        public ProjectionOptions(Profile profile) { this(profile, null); }
        public Map<String, Object> toMap() {
            Map<String, Object> result = fields("profile", profile.wireName());
            if (features != null) result.put("features", mapped(features, ProjectionFeature::wireName));
            return result;
        }
    }

    private static <T, R> List<R> mapped(List<T> values, Function<T, R> convert) {
        List<R> result = new ArrayList<>();
        for (T value : values) result.add(convert.apply(value));
        return java.util.Collections.unmodifiableList(result);
    }

    private static Map<String, Object> fields(Object... pairs) {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < pairs.length; i += 2) result.put((String)pairs[i], pairs[i + 1]);
        return result;
    }

    @SuppressWarnings("unchecked")
    private static Map<String, Object> object(Object value) { return (Map<String, Object>)value; }

    private static Long time(Object value) { return value == null ? null : ((Number)value).longValue(); }

    private static <T> List<T> records(Object value, Function<Map<String, Object>, T> decode) {
        if (value == null) return java.util.Collections.emptyList();
        List<T> result = new ArrayList<>();
        for (Object item : (List<?>)value) result.add(decode.apply(object(item)));
        return result;
    }

    @SuppressWarnings("unchecked")
    private static List<String> strings(Object value) { return value == null ? java.util.Collections.emptyList() : (List<String>)value; }

    private static void optional(Map<String, Object> map, String key, Object value) {
        if (value != null && (!(value instanceof List<?> list) || !list.isEmpty())) map.put(key, value);
    }

    public record Segment(String text, Long startUs, Long endUs, Map<String, Object> attributes, String id, String language, List<String> agentIds, List<String> roles) {
        public Segment(String text) { this(text, null, null, java.util.Collections.emptyMap(), null, null, java.util.Collections.emptyList(), java.util.Collections.emptyList()); }

        static Segment fromMap(Map<String, Object> m) {
            return new Segment((String)m.get("text"), time(m.get("start_us")), time(m.get("end_us")), object(m.get("attributes")), (String)m.get("id"), (String)m.get("language"), strings(m.get("agent_ids")), strings(m.get("roles")));
        }

        public Map<String, Object> toMap() {
            var result = fields("text", text, "start_us", startUs, "end_us", endUs, "attributes", attributes);
            optional(result, "id", id);
            optional(result, "language", language);
            optional(result, "agent_ids", agentIds);
            optional(result, "roles", roles);
            return result;
        }
    }

    public record Cue(String id, Long startUs, Long endUs, List<Segment> segments, Map<String, Object> attributes, String language, List<String> agentIds, List<String> roles, String parentId) {
        public Cue(String id, List<Segment> segments) { this(id, null, null, segments, java.util.Collections.emptyMap(), null, java.util.Collections.emptyList(), java.util.Collections.emptyList(), null); }

        static Cue fromMap(Map<String, Object> m) {
            return new Cue((String)m.get("id"), time(m.get("start_us")), time(m.get("end_us")), records(m.get("segments"), Segment::fromMap), object(m.get("attributes")), (String)m.get("language"), strings(m.get("agent_ids")), strings(m.get("roles")), (String)m.get("parent_id"));
        }

        public Map<String, Object> toMap() {
            var result = fields("id", id, "start_us", startUs, "end_us", endUs, "segments", mapped(segments, Segment::toMap), "attributes", attributes);
            optional(result, "language", language);
            optional(result, "agent_ids", agentIds);
            optional(result, "roles", roles);
            optional(result, "parent_id", parentId);
            return result;
        }
    }

    public record Agent(String id, String name, String kind, Map<String, Object> attributes) {
        public Agent(String id) { this(id, null, null, java.util.Collections.emptyMap()); }

        static Agent fromMap(Map<String, Object> m) {
            return new Agent((String)m.get("id"), (String)m.get("name"), (String)m.get("kind"), object(m.get("attributes")));
        }

        public Map<String, Object> toMap() {
            var result = fields("id", id, "attributes", attributes);
            optional(result, "name", name);
            optional(result, "kind", kind);
            return result;
        }
    }

    public record Auxiliary(String id, String kind, String language, List<String> targetIds, List<Segment> segments, Map<String, Object> attributes) {
        public Auxiliary(String id, String kind, List<String> targetIds, List<Segment> segments) { this(id, kind, null, targetIds, segments, java.util.Collections.emptyMap()); }

        static Auxiliary fromMap(Map<String, Object> m) {
            return new Auxiliary((String)m.get("id"), (String)m.get("kind"), (String)m.get("language"), strings(m.get("target_ids")), records(m.get("segments"), Segment::fromMap), object(m.get("attributes")));
        }

        public Map<String, Object> toMap() {
            var result = fields("id", id, "kind", kind, "target_ids", targetIds, "segments", mapped(segments, Segment::toMap), "attributes", attributes);
            optional(result, "language", language);
            return result;
        }
    }

    public record Metadata(String key, String value) {
        static Metadata fromMap(Map<String, Object> m) {
            return new Metadata((String)m.get("key"), (String)m.get("value"));
        }

        public Map<String, Object> toMap() {
            var result = fields("key", key, "value", value);
            return result;
        }
    }

    public record Document(int version, List<Cue> cues, List<Metadata> metadata, Map<String, Object> attributes, String language, List<Agent> agents, List<Auxiliary> auxiliary) {
        public Document(List<Cue> cues) { this(1, cues, java.util.Collections.emptyList(), java.util.Collections.emptyMap(), null, java.util.Collections.emptyList(), java.util.Collections.emptyList()); }

        static Document fromMap(Map<String, Object> m) {
            return new Document(((Number)m.get("version")).intValue(), records(m.get("cues"), Cue::fromMap), records(m.get("metadata"), Metadata::fromMap), object(m.get("attributes")), (String)m.get("language"), records(m.get("agents"), Agent::fromMap), records(m.get("auxiliary"), Auxiliary::fromMap));
        }

        public Map<String, Object> toMap() {
            var result = fields("version", version, "cues", mapped(cues, Cue::toMap), "metadata", mapped(metadata, Metadata::toMap), "attributes", attributes);
            optional(result, "language", language);
            optional(result, "agents", mapped(agents, Agent::toMap));
            optional(result, "auxiliary", mapped(auxiliary, Auxiliary::toMap));
            return result;
        }
    }

    public record Diagnostic(String code, String message, String cueId) {
        static Diagnostic fromMap(Map<String, Object> m) {
            return new Diagnostic((String)m.get("code"), (String)m.get("message"), (String)m.get("cue_id"));
        }

        public Map<String, Object> toMap() {
            var result = fields("code", code, "message", message, "cue_id", cueId);
            return result;
        }
    }

    public record ParseResult(Document document, List<Diagnostic> diagnostics) {
        static ParseResult fromMap(Map<String, Object> m) {
            return new ParseResult(Document.fromMap(object(m.get("document"))), records(m.get("diagnostics"), Diagnostic::fromMap));
        }

        public Map<String, Object> toMap() {
            var result = fields("document", document.toMap(), "diagnostics", mapped(diagnostics, Diagnostic::toMap));
            return result;
        }
    }

    /** Canonical snapshot paired to exact output/format; not authenticated provenance. */
    public record PreservationSidecar(long version, Format format, String output, Document document) {
        public PreservationSidecar(Format format, String output, Document document) { this(1, format, output, document); }
        public static PreservationSidecar fromMap(Map<String, Object> m) {
            // Validate before typed decoding can normalize numbers or omit unknown fields.
            UniLyrics.restorePreserved((String)m.get("output"), (String)m.get("format"), m);
            return new PreservationSidecar(((Number)m.get("version")).longValue(), Format.fromWireName((String)m.get("format")), (String)m.get("output"), Document.fromMap(object(m.get("document"))));
        }
        public Map<String, Object> toMap() {
            return fields("version", version, "format", format.wireName(), "output", output, "document", document.toMap());
        }
    }

    /** Save output and sidecar together. Losses describe output alone. */
    public record PreservedExportResult(String output, List<Diagnostic> losses, PreservationSidecar sidecar) {
        public static PreservedExportResult fromMap(Map<String, Object> m) {
            return new PreservedExportResult((String)m.get("output"), records(m.get("losses"), Diagnostic::fromMap), PreservationSidecar.fromMap(object(m.get("sidecar"))));
        }
        public Map<String, Object> toMap() {
            return fields("output", output, "losses", mapped(losses, Diagnostic::toMap), "sidecar", sidecar.toMap());
        }
    }

    public record ExportResult(String output, List<Diagnostic> losses) {
        static ExportResult fromMap(Map<String, Object> m) {
            return new ExportResult((String)m.get("output"), records(m.get("losses"), Diagnostic::fromMap));
        }

        public Map<String, Object> toMap() {
            var result = fields("output", output, "losses", mapped(losses, Diagnostic::toMap));
            return result;
        }
    }

    public record Features(boolean lineTiming, boolean segmentTiming, boolean explicitEnds, boolean agents, boolean relationships, List<String> languages, List<String> roles, List<String> auxiliaryKinds, boolean extensions) {
        static Features fromMap(Map<String, Object> m) {
            return new Features((Boolean)m.get("line_timing"), (Boolean)m.get("segment_timing"), (Boolean)m.get("explicit_ends"), (Boolean)m.get("agents"), (Boolean)m.get("relationships"), strings(m.get("languages")), strings(m.get("roles")), strings(m.get("auxiliary_kinds")), (Boolean)m.get("extensions"));
        }

        public Map<String, Object> toMap() {
            var result = fields("line_timing", lineTiming, "segment_timing", segmentTiming, "explicit_ends", explicitEnds, "agents", agents, "relationships", relationships, "languages", languages, "roles", roles, "auxiliary_kinds", auxiliaryKinds, "extensions", extensions);
            return result;
        }
    }
}
