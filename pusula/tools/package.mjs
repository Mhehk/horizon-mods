// Packages mixins/pusula as a source-only .hnmod (deterministic: fixed dates,
// sorted entries, LF line endings).
//   node tools/package.mjs [--install]
// Output: target/pusula-16.10.1.hnmod; --install also copies it to
// %APPDATA%\horizon\horizon-native-mods\pusula.hnmod.
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bundle = path.join(root, "mixins", "pusula");
const compat = JSON.parse(fs.readFileSync(path.join(root, "compatibility.json"), "utf8"));
const toml = fs.readFileSync(path.join(bundle, "horizon-mixin.toml"), "utf8");
const field = (re) => (toml.match(re) || [])[1];
const id = field(/^id\s*=\s*"([^"]+)"/m);
const version = field(/^version\s*=\s*"([^"]+)"/m);
const pin = field(/^horizon_revision\s*=\s*"([^"]+)"/m);
if (pin !== compat.horizon_revision) {
  console.error("horizon-mixin.toml and compatibility.json pin different Horizon revisions");
  process.exit(1);
}
const manifest = { game: compat.game, horizon: compat.horizon_revision, id, mixins: compat.horizon_mixins_revision, schema: 1, version };
if (compat.hooks !== undefined) manifest.hooks = compat.hooks;
const sorted = Object.fromEntries(Object.keys(manifest).sort().map((k) => [k, manifest[k]]));
const files = [["horizon-native.json", Buffer.from(JSON.stringify(sorted, null, 2) + "\n")]];
const walk = (dir) => fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
  const p = path.join(dir, e.name);
  return e.isDirectory() ? walk(p) : [p];
});
for (const p of walk(bundle).filter((p) => /\.(rs|toml)$/.test(p))) {
  if (path.basename(p) === "Cargo.lock") throw new Error("remove " + p);
  const rel = path.relative(bundle, p).split(path.sep).join("/");
  const text = fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n");
  files.push([`mixins/pusula/${rel}`, Buffer.from(text)]);
}
files.sort((a, b) => (a[0] < b[0] ? -1 : 1));

// Minimal zip writer (deflate), DOS time 2026-01-01 00:00.
const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (b) => {
  let c = 0xffffffff;
  for (const x of b) c = crcTable[(c ^ x) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const dosTime = 0, dosDate = ((2026 - 1980) << 9) | (1 << 5) | 1;
const locals = [], centrals = [];
let offset = 0;
for (const [name, data] of files) {
  const nameBuf = Buffer.from(name, "utf8");
  const comp = zlib.deflateRawSync(data, { level: 9 });
  const crc = crc32(data);
  const lh = Buffer.alloc(30);
  lh.writeUInt32LE(0x04034b50, 0); lh.writeUInt16LE(20, 4); lh.writeUInt16LE(0x0800, 6); lh.writeUInt16LE(8, 8);
  lh.writeUInt16LE(dosTime, 10); lh.writeUInt16LE(dosDate, 12); lh.writeUInt32LE(crc, 14);
  lh.writeUInt32LE(comp.length, 18); lh.writeUInt32LE(data.length, 22); lh.writeUInt16LE(nameBuf.length, 26);
  locals.push(lh, nameBuf, comp);
  const ch = Buffer.alloc(46);
  ch.writeUInt32LE(0x02014b50, 0); ch.writeUInt16LE((3 << 8) | 20, 4); ch.writeUInt16LE(20, 6); ch.writeUInt16LE(0x0800, 8);
  ch.writeUInt16LE(8, 10); ch.writeUInt16LE(dosTime, 12); ch.writeUInt16LE(dosDate, 14); ch.writeUInt32LE(crc, 16);
  ch.writeUInt32LE(comp.length, 20); ch.writeUInt32LE(data.length, 24); ch.writeUInt16LE(nameBuf.length, 28);
  ch.writeUInt32LE((0o100644 << 16) >>> 0, 38); ch.writeUInt32LE(offset, 42);
  centrals.push(ch, nameBuf);
  offset += 30 + nameBuf.length + comp.length;
}
const cd = Buffer.concat(centrals);
const end = Buffer.alloc(22);
end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(files.length, 8); end.writeUInt16LE(files.length, 10);
end.writeUInt32LE(cd.length, 12); end.writeUInt32LE(offset, 16);
const zip = Buffer.concat([...locals, cd, end]);

const outDir = path.join(root, "target");
fs.mkdirSync(outDir, { recursive: true });
const out = path.join(outDir, "pusula-16.10.1.hnmod");
fs.writeFileSync(out, zip);
const result = { file: out, files: files.length, size: zip.length, sha256: crypto.createHash("sha256").update(zip).digest("hex") };
if (process.argv.includes("--install")) {
  const dest = path.join(process.env.APPDATA, "horizon", "horizon-native-mods", `${id}.hnmod`);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(out, dest);
  result.installed = dest;
}
console.log(JSON.stringify(result, null, 2));
