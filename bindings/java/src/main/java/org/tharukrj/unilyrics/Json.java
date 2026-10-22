package org.tharukrj.unilyrics;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Use the JSON API common to Android and desktop JSON-java. */
final class Json {
    private Json() {}

    static Map<String, Object> request(Object... pairs) {
        if (pairs.length % 2 != 0) throw new IllegalArgumentException("Request fields need key/value pairs");
        Map<String, Object> result = new LinkedHashMap<>();
        for (int index = 0; index < pairs.length; index += 2) {
            String key = (String)java.util.Objects.requireNonNull(pairs[index]);
            Object value = java.util.Objects.requireNonNull(pairs[index + 1]);
            if (result.put(key, value) != null) throw new IllegalArgumentException("Duplicate request field: " + key);
        }
        return java.util.Collections.unmodifiableMap(result);
    }

    static String encode(Object value) {
        try { return encodeValue(value, 0).toString(); }
        catch (JSONException error) { throw new IllegalArgumentException("Cannot encode JSON", error); }
    }

    private static Object encodeValue(Object value, int depth) throws JSONException {
        checkDepth(depth);
        if (value == null) return JSONObject.NULL;
        if (value instanceof Map<?, ?> map) {
            var object = new JSONObject();
            for (var entry : map.entrySet()) {
                if (!(entry.getKey() instanceof String)) throw new IllegalArgumentException("JSON keys must be strings");
                object.put((String)entry.getKey(), encodeValue(entry.getValue(), depth + 1));
            }
            return object;
        }
        if (value instanceof List<?> list) {
            var array = new JSONArray();
            for (var item : list) array.put(encodeValue(item, depth + 1));
            return array;
        }
        if (value instanceof String || value instanceof Number || value instanceof Boolean) return value;
        throw new IllegalArgumentException("Values must be JSON maps, lists, strings, numbers, booleans or null");
    }

    static Map<String, Object> decode(String text) {
        try { return UniLyrics.objectResult(decodeValue(new JSONObject(text), 0)); }
        catch (JSONException error) { throw new IllegalArgumentException("Cannot decode JSON", error); }
    }

    private static Object decodeValue(Object value, int depth) throws JSONException {
        checkDepth(depth);
        if (value == JSONObject.NULL) return null;
        if (value instanceof JSONObject object) {
            Map<String, Object> result = new LinkedHashMap<>();
            var keys = object.keys();
            while (keys.hasNext()) {
                String key = keys.next();
                result.put(key, decodeValue(object.get(key), depth + 1));
            }
            return result;
        }
        if (value instanceof JSONArray array) {
            var result = new ArrayList<Object>();
            for (int index = 0; index < array.length(); index++) result.add(decodeValue(array.get(index), depth + 1));
            return result;
        }
        return value;
    }

    private static void checkDepth(int depth) {
        if (depth > 128) throw new IllegalArgumentException("JSON nesting exceeds 128 levels");
    }
}
