// Vendored from webm-writer@1.0.0 (MIT, thenickdude) — the npm package's
// browser entry is CommonJS and Vite's rollup build left its default
// import as an unresolved global, so we wrap the files directly as ESM.
import makeWebMWriter from "./WebMWriter.js";
import ArrayBufferDataStream from "./ArrayBufferDataStream.js";
import BlobBuffer from "./BlobBuffer.js";

const WebMWriter = makeWebMWriter(ArrayBufferDataStream, BlobBuffer(null));
export default WebMWriter;
