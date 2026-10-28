package org.tharukrj.unilyrics;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import org.json.JSONArray;
import org.json.JSONObject;

/** Shared authored fixtures executed through the installed Android app's JNI. */
public final class AndroidFixtureTest {
    private static void equal(Object actual, Object expected) throws Exception {
        var left = Json.decode(new JSONObject().put("value", actual).toString());
        var right = Json.decode(new JSONObject().put("value", expected).toString());
        if (!left.equals(right)) throw new AssertionError("Fixture mismatch: " + left + " != " + right);
    }

    private static JSONObject invoke(JSONObject request, JSONObject test) throws Exception {
        var reply = new JSONObject(UniLyrics.process(request.toString()));
        if (test.has("error")) {
            if (reply.getBoolean("ok")) throw new AssertionError("Expected fixture failure");
            equal(reply.getJSONObject("error").getString("code"), test.getString("error"));
            return null;
        }
        if (!reply.getBoolean("ok")) throw new AssertionError(reply.toString());
        return reply.getJSONObject("result");
    }

    public static void run(String kind, InputStream input) throws Exception {
        StringBuilder text = new StringBuilder();
        try (var reader = new BufferedReader(new InputStreamReader(input, StandardCharsets.UTF_8))) {
            for (String line; (line = reader.readLine()) != null;) text.append(line).append('\n');
        }
        var cases = new JSONArray(text.toString());
        for (int i = 0; i < cases.length(); i++) {
            var test = cases.getJSONObject(i);
            if (kind.equals("conformance")) {
                String source = test.has("input") ? test.getString("input") : test.getJSONObject("document").toString();
                var result = invoke(new JSONObject().put("op", "parse").put("input", source)
                    .put("format", test.getString("format")), test);
                if (result == null) continue;
                var document = result.getJSONObject("document");
                if (test.has("document")) equal(document, test.getJSONObject("document"));
                var cues = document.getJSONArray("cues");
                var texts = new JSONArray();
                var starts = new JSONArray();
                for (int j = 0; j < cues.length(); j++) {
                    var cue = cues.getJSONObject(j);
                    StringBuilder content = new StringBuilder();
                    var segments = cue.getJSONArray("segments");
                    for (int k = 0; k < segments.length(); k++) content.append(segments.getJSONObject(k).getString("text"));
                    texts.put(content.toString());
                    starts.put(cue.get("start_us"));
                }
                equal(texts, test.getJSONArray("text"));
                equal(starts, test.getJSONArray("starts"));
                if (test.has("features")) {
                    var features = invoke(new JSONObject().put("op", "inspect").put("document", document), new JSONObject());
                    equal(features, test.getJSONObject("features"));
                    equal(invoke(new JSONObject().put("op", "validate").put("document", document), new JSONObject()),
                          new JSONObject().put("valid", true));
                }
                var exports = test.optJSONArray("exports");
                if (exports != null) for (int j = 0; j < exports.length(); j++) {
                    var expected = exports.getJSONObject(j);
                    var exported = invoke(new JSONObject().put("op", "export").put("document", document)
                        .put("format", expected.getString("format")).put("policy", expected.getString("policy")), new JSONObject());
                    equal(exported.getString("output"), expected.getString("output"));
                    equal(exported.getJSONArray("losses"), new JSONArray());
                }
            } else {
                JSONObject request;
                if (kind.equals("normalization")) {
                    request = new JSONObject().put("op", "normalize").put("document", test.getJSONObject("document"));
                    if (test.has("offset_us")) request.put("offset_us", test.get("offset_us"));
                } else request = test.getJSONObject("request");
                var result = invoke(request, test);
                if (result == null) continue;
                if (test.has("expected_document")) equal(result.getJSONObject("document"), test.getJSONObject("expected_document"));
                else equal(new JSONObject(result.getString("output")), test.getJSONObject("output_document"));
                var diagnostics = result.has("diagnostics") ? result.getJSONArray("diagnostics") : result.getJSONArray("losses");
                var codes = new JSONArray();
                for (int j = 0; j < diagnostics.length(); j++) codes.put(diagnostics.getJSONObject(j).getString("code"));
                equal(codes, test.getJSONArray(kind.equals("normalization") ? "diagnostic_codes" : "codes"));
            }
        }
        System.out.println("Installed Android " + kind + " fixtures passed: " + cases.length());
    }
}
