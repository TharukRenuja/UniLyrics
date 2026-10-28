import { initialize, Registry, type Document, type ParseResult } from "./index.js";

async function example(bytes: Uint8Array): Promise<Document> {
  const sdk = await initialize(bytes);
  const registry = new Registry(sdk);
  registry.register("x-custom", {parse: input => sdk.parse(input, "plain"), export: (document, policy) => sdk.export(document, "plain", policy)});
  const custom: ParseResult = registry.parse("Hello", "x-custom");
  registry.export(custom.document, "x-custom", "strict");
  // @ts-expect-error custom callbacks must return a complete parse result
  registry.register("x-invalid", {parse: () => ({document: custom.document})});
  // @ts-expect-error asynchronous callbacks are unsupported
  registry.register("x-async", {parse: async input => sdk.parse(input)});
  sdk.parse("1\n00:00:01,000 --> 00:00:02,000\nvoice", "srt");
  sdk.parse("WEBVTT\n\n00:01.000 --> 00:02.000\nvoice", "webvtt");
  const parsed: ParseResult = sdk.parse("[00:01]Hello", "lrc");
  const output: string = sdk.export(parsed.document, "json", "strict").output;
  sdk.validate(sdk.parse(output, "json").document);
  parsed.document.agents = [{id:"singer", kind:"person", attributes:{}}];
  parsed.document.auxiliary = [{id:"translation", kind:"translation", target_ids:[parsed.document.cues[0].id], segments:[], attributes:{}}];
  const bundle = sdk.exportPreserved(parsed.document, "plain", {profile: "plain"});
  const restored: ParseResult = sdk.restorePreserved(bundle.output, "plain", bundle.sidecar);
  // @ts-expect-error sidecar is mandatory and structurally typed
  sdk.restorePreserved(bundle.output, "plain", {});
  void restored;
  const projected: ParseResult = sdk.project(parsed.document, {profile: "custom", features: []});
  sdk.exportProfile(projected.document, "json", {profile: "full"}, "strict");
  // @ts-expect-error Unsupported profiles must not type-check.
  sdk.project(parsed.document, {profile: "invented"});
  // @ts-expect-error Unsupported feature names must not type-check.
  sdk.project(parsed.document, {profile: "custom", features: ["invented"]});
  const shifted = sdk.normalize(parsed.document, -250000);
  const shiftedStart: number | null = shifted.document.cues[0].start_us;
  void shiftedStart;
  // @ts-expect-error offsets are signed integer microseconds, not strings
  sdk.normalize(parsed.document, "100");
  const languages: string[] = sdk.inspect(parsed.document).languages;
  void languages;
  // @ts-expect-error Unsupported format names must not type-check.
  sdk.parse("Hello", "invented-format");
  return parsed.document;
}
void example;
