import {initialize} from "../bindings/javascript/index.js";
import type {Document} from "../bindings/javascript/index.js";

// Supply WASM bytes from your app's file loader or browser fetch.
export async function parseExample(wasm: Uint8Array): Promise<Document> {
    const sdk = await initialize(wasm);
    const document: Document = sdk.parse("[00:01]Hello 世界 🎵", "lrc").document;
    return sdk.parse(sdk.export(document, "json", "strict").output, "json").document;
}
