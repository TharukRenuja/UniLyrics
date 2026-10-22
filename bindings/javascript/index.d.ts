export type Profile = "full" | "line" | "word" | "karaoke" | "plain" | "translated" | "custom";
export type ProjectionFeature = "line_timing" | "segment_timing" | "agents" | "relationships" | "languages" | "roles" | "metadata" | "attributes" | "auxiliary";
export interface ProjectionOptions { profile?: Profile; features?: ProjectionFeature[] }
export type Format = "plain" | "lrc" | "enhanced_lrc" | "json" | "ttml" | "ttml_lyrics" | "srt" | "webvtt";
export type Policy = "strict" | "compatible";
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export interface Segment {
  id?: string;
  language?: string;
  agent_ids?: string[];
  roles?: string[];
  text: string;
  start_us: number | null;
  end_us: number | null;
  attributes: Record<string, Json>;
}
export interface Cue {
  language?: string;
  agent_ids?: string[];
  roles?: string[];
  parent_id?: string;
  id: string;
  start_us: number | null;
  end_us: number | null;
  segments: Segment[];
  attributes: Record<string, Json>;
}
export interface Agent {
  id: string;
  name?: string;
  kind?: string;
  attributes: Record<string, Json>;
}
export interface Auxiliary {
  id: string;
  kind: string;
  language?: string;
  target_ids: string[];
  segments: Segment[];
  attributes: Record<string, Json>;
}
export interface Features {
  line_timing: boolean;
  segment_timing: boolean;
  explicit_ends: boolean;
  agents: boolean;
  relationships: boolean;
  languages: string[];
  roles: string[];
  auxiliary_kinds: string[];
  extensions: boolean;
}
export interface Document {
  language?: string;
  agents?: Agent[];
  auxiliary?: Auxiliary[];
  version: 1;
  cues: Cue[];
  metadata: { key: string; value: string }[];
  attributes: Record<string, Json>;
}
export interface Diagnostic { code: string; message: string; cue_id: string | null }
export interface ParseResult { document: Document; diagnostics: Diagnostic[] }
export interface PreservationSidecar { version: 1; format: Format; output: string; document: Document }
export interface PreservedExportResult { output: string; losses: Diagnostic[]; sidecar: PreservationSidecar }
export interface ExportResult { output: string; losses: Diagnostic[] }
export class UniLyricsError extends Error { code: string; constructor(code: string, message: string) }
export interface UniLyrics {
  process(request: Json): Json;
  parse(input: string, format?: Format | null): ParseResult;
  detect(input: string): Format;
  export(document: Document, format: Format, policy?: Policy): ExportResult;
  validate(document: Document): { valid: true };
  inspect(document: Document): Features;
  exportPreserved(document: Document, format: Format, options?: ProjectionOptions): PreservedExportResult;
  restorePreserved(input: string, format: Format, sidecar: PreservationSidecar): ParseResult;
  project(document: Document, options?: ProjectionOptions): ParseResult;
  exportProfile(document: Document, format: Format, options?: ProjectionOptions, policy?: Policy): ExportResult;
  normalize(document: Document, offset_us?: number): ParseResult;
}
export function initialize(source: BufferSource | WebAssembly.Module): Promise<UniLyrics>;

export type RegistryFormat = Format | `x-${string}`;
export interface CustomFormat {
  matches?(input: string): boolean;
  parse?(input: string): ParseResult;
  export?(document: Document, policy: Policy): ExportResult;
}
export class Registry {
  constructor(engine: UniLyrics);
  register(name: string, adapter: CustomFormat): void;
  detect(input: string): RegistryFormat;
  parse(input: string, format?: RegistryFormat | null): ParseResult;
  export(document: Document, format: RegistryFormat, policy?: Policy): ExportResult;
}
