const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export class UniLyricsError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "UniLyricsError";
    this.code = code;
  }
}

/** Accept compiled WebAssembly (including Workers imports) or the module's bytes. */
export async function initialize(source) {
  const loaded = await WebAssembly.instantiate(source, {});
  const core = (loaded instanceof WebAssembly.Instance ? loaded : loaded.instance).exports;
  for (const name of ["unilyrics_process", "unilyrics_free", "unilyrics_alloc", "unilyrics_free_input"]) {
    if (typeof core[name] !== "function") throw new TypeError(`Missing WASM export: ${name}`);
  }
  if (!(core.memory instanceof WebAssembly.Memory)) throw new TypeError("Missing WASM memory");

  function process(request) {
    const data = encoder.encode(JSON.stringify(request, (_key, value) => {
      if (["undefined", "function", "symbol"].includes(typeof value) || (typeof value === "number" && !Number.isFinite(value))) {
        throw new TypeError("Request must contain only JSON values");
      }
      return value;
    }));
    if (data.length > 1048576) throw new UniLyricsError("INPUT_TOO_LARGE", "Request exceeds 1 MiB.");
    const input = core.unilyrics_alloc(data.length);
    if (!input) throw new Error("Could not allocate request buffer");
    let output = 0;
    try {
      new Uint8Array(core.memory.buffer, input, data.length).set(data);
      output = core.unilyrics_process(input, data.length);
      if (!output) throw new Error("UniLyrics returned no response");
      const memory = new Uint8Array(core.memory.buffer);
      const end = memory.indexOf(0, output);
      if (end < 0) throw new Error("Unterminated UniLyrics response");
      const response = JSON.parse(decoder.decode(memory.subarray(output, end)));
      if (!response.ok) throw new UniLyricsError(response.error.code, response.error.message);
      return response.result;
    } finally {
      if (output) core.unilyrics_free(output);
      core.unilyrics_free_input(input, data.length);
    }
  }

  return {
    process,
    parse: (input, format = null) => process({ op: "parse", input, format }),
    detect: input => process({ op: "detect", input }),
    export: (document, format, policy = "compatible") => process({ op: "export", document, format, policy }),
    validate: document => process({ op: "validate", document }),
    inspect: document => process({ op: "inspect", document }),
    exportPreserved: (document, format, options = {}) => {
      const result = process({ op: "export_preserved", document, format, options });
      const restore = { op: "restore_preserved", input: result.output, format, sidecar: result.sidecar };
      if (encoder.encode(JSON.stringify(restore)).length > 1048576) throw new UniLyricsError("INPUT_TOO_LARGE", "Encoded preservation restore request exceeds 1 MiB");
      return result;
    },
    restorePreserved: (input, format, sidecar) => process({ op: "restore_preserved", input, format, sidecar }),
    project: (document, options = {}) => process({ op: "project", document, options }),
    exportProfile: (document, format, options = {}, policy = "compatible") => process({ op: "export_profile", document, format, options, policy }),
    normalize: (document, offset_us = 0) => process({ op: "normalize", document, offset_us }),
  };
}

const builtinFormats = new Set(["plain", "lrc", "enhanced_lrc", "json", "ttml", "ttml_lyrics", "srt", "webvtt"]);

/** Scoped synchronous application callbacks; core validation and loss checks stay in Rust. */
export class Registry {
  #engine;
  #formats = new Map();
  constructor(engine) { this.#engine = engine; }

  register(name, adapter) {
    if (typeof name !== "string" || !name.startsWith("x-") || name.length < 3 || name.length > 64 || /[^a-z0-9_-]/.test(name)) {
      throw new UniLyricsError("INVALID_CUSTOM_FORMAT", "Use a 3–64 character lowercase ASCII name in the x- namespace.");
    }
    if (this.#formats.has(name)) throw new UniLyricsError("DUPLICATE_FORMAT", "Custom format is already registered.");
    if (!adapter || typeof adapter !== "object") throw new TypeError("Expected a custom format adapter");
    const callbacks = {};
    for (const key of ["parse", "export", "matches"]) {
      if (adapter[key] !== undefined) {
        if (typeof adapter[key] !== "function") throw new TypeError(`${key} must be a function`);
        callbacks[key] = adapter[key].bind(adapter);
      }
    }
    this.#formats.set(name, callbacks);
  }

  #checkInput(input) {
    if (typeof input !== "string") throw new TypeError("Input must be a string");
    const data = encoder.encode(input);
    if (decoder.decode(data) !== input) throw new UniLyricsError("INVALID_UTF8", "Input contains invalid Unicode.");
    if (data.length > 1048576) throw new UniLyricsError("INPUT_TOO_LARGE", "Input exceeds 1 MiB.");
  }

  detect(input) {
    this.#checkInput(input);
    let builtin, error;
    try { builtin = this.#engine.detect(input); }
    catch (caught) {
      if (!(caught instanceof UniLyricsError)) throw caught;
      error = caught;
    }
    const matches = [];
    for (const [name, adapter] of this.#formats) {
      if (!adapter.matches) continue;
      const matched = adapter.matches(input);
      if (typeof matched !== "boolean") throw new TypeError("Custom detector must return a boolean synchronously");
      if (matched) matches.push(name);
    }
    if (matches.length > 1 || (matches.length && builtin && builtin !== "plain")) {
      throw new UniLyricsError("AMBIGUOUS_FORMAT", "Multiple formats match; choose explicitly.");
    }
    if (matches.length) return matches[0];
    if (error) throw error;
    return builtin;
  }

  parse(input, format = null) {
    this.#checkInput(input);
    const name = format ?? this.detect(input);
    const adapter = this.#formats.get(name);
    if (!adapter) {
      if (!builtinFormats.has(name)) throw new UniLyricsError("UNKNOWN_FORMAT", "Format is not registered.");
      return this.#engine.parse(input, name);
    }
    if (!adapter.parse) throw new UniLyricsError("UNSUPPORTED_OPERATION", "Custom format has no parser.");
    return this.#engine.process({op: "validate_custom_parse", result: adapter.parse(input)});
  }

  export(document, format, policy = "compatible") {
    const adapter = this.#formats.get(format);
    if (!adapter) {
      if (!builtinFormats.has(format)) throw new UniLyricsError("UNKNOWN_FORMAT", "Format is not registered.");
      return this.#engine.export(document, format, policy);
    }
    if (!["strict", "compatible"].includes(policy)) throw new UniLyricsError("INVALID_POLICY", "Unknown export policy.");
    const canonical = this.#engine.export(document, "json", "strict").output;
    if (!adapter.export) throw new UniLyricsError("UNSUPPORTED_OPERATION", "Custom format has no exporter.");
    const result = this.#engine.process({op: "validate_custom_export", result: adapter.export(JSON.parse(canonical), policy)});
    let roundtrip = null;
    if (adapter.parse) {
      try { roundtrip = this.#engine.process({op: "validate_custom_parse", result: adapter.parse(result.output)}).document; }
      catch { /* Failed callback reparsing is reported by shared verification. */ }
    }
    return this.#engine.process({op: "verify_custom_export", document: JSON.parse(canonical), result, roundtrip, policy});
  }
}
