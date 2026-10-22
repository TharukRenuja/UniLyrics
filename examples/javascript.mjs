import {readFile} from "node:fs/promises";
import {initialize} from "../bindings/javascript/index.js";

const sdk = await initialize(await readFile(new URL("../bindings/javascript/dist/core.wasm", import.meta.url)));
const {document} = sdk.parse("[00:01]Hello 世界 🎵", "lrc");
console.log(sdk.export(document, "json", "strict").output);
