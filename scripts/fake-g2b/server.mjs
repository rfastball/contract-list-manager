// 가짜 나라장터 — 127.0.0.1 의 HTTPS 정적 서버(node 표준 모듈만). scripts/fake-g2b.ps1 이 띄운다.
//
//   node scripts/fake-g2b/server.mjs <pfx> <포트> <pfx 암호> <개발 홈>   서버 + 확장 복사본 지키기
//   node scripts/fake-g2b/server.mjs --id                      개발용 확장 ID 를 적고 끝낸다
//   node scripts/fake-g2b/server.mjs --spki <cer>              인증서 공개키의 sha256(base64)을 적고 끝낸다
//
// 시작할 때 extension/ 을 개발 홈의 extension/ 으로 뜨되 — 업무 홈의 「확장 준비」 가 꺼내는 자리 · 꼴 그대로라 창이 그 홈의
// 확장 상태를 읽는다 — manifest 의 key 만 개발용으로 바꾸고 이름에 「(가짜 나라장터)」 를 붙인다. 그 뒤 extension/ 이 바뀌면 다시 뜨고 묶음 지문을 게시한다(패널이 닫히면 확장이 자동 적용).
import { createServer } from "node:https";
import { createHash, X509Certificate } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");

// 개발용 확장의 공개키(SPKI DER)와 그 ID. 개인키는 만들자마자 버렸다 — Chrome 은 압축 해제 확장의 ID 를 key 로만 정한다.
// 확장은 제 ID 가 이 값일 때만 개발 호스트에 잇는다(extension/background.js 의 DEVELOPMENT_ID — 시험이 대조한다).
// 복사본이 진짜 확장의 ID 를 가지면 업무용 호스트에 붙어 지어낸 건이 실제 작업자료에 저장된다 — ID 가 달라야 업무용 호스트의 allowed_origins 가 막는다.
const DEV_KEY = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA7kyg7LM2N6a9s3eUcLq5o55lXGL/YDa1OyneQo1yrcJJfAA2icy8YtQBa2n9IWCm+Z+1LKRqYBHff82UYV46z0/OxTdjdC/llGenzDSfaMmLMi2AQ2lQ8MEHMnzM6nR824sQguz13ufm00u4eDwIiM2Hf/gbHvi5pLWZx9XSmrm2S//YigD2yStGVJ9+qq6ccOZBD7/CcSeorjm8C2TIkiP/oNAx9cIS/u/8EYmRHWfBuNTv07kCbAO859NVDwFx2JAPpBrhxRMIqbvwih8h/SI1zjg+e6eXgrCTdiYaiKoPq39U3DLiaktG7zbICJhprAFqoeoIJZU+lShYos6elwIDAQAB";
const DEV_ID = "kjjcbnphdfamcohjlfipcnbodbblkffc";

const source = path.join(root, "extension");
const sha256 = bytes => createHash("sha256").update(bytes).digest();
const REVISION_TOKEN = "__PCLM_BUNDLE_REVISION__";
const packedKinds = new Set([".json", ".js", ".html", ".css", ".wasm", ".hwpx", ".txt"]);
// 확장 ID — 공개키 sha256 의 앞 16바이트를 16진 한 자리씩 a–p 로.
const idOf = key => [...sha256(Buffer.from(key, "base64")).subarray(0, 16).toString("hex")].map(h => "abcdefghijklmnop"[parseInt(h, 16)]).join("");
const manifestOf = text => {
  const m = JSON.parse(text.replace(/^﻿/, ""));
  m.key = DEV_KEY;
  m.name += " (가짜 나라장터)";
  return JSON.stringify(m, null, 2) + "\n";
};
function packedFiles(folder, prefix = "") {
  return fs.readdirSync(folder, { withFileTypes: true }).flatMap(entry => {
    const relative = `${prefix}${entry.name}`;
    if (entry.isDirectory()) return packedFiles(path.join(folder, entry.name), `${relative}/`);
    return entry.isFile() && packedKinds.has(path.extname(entry.name).toLowerCase()) ? [relative] : [];
  }).sort();
}
function snapshotExtension() {
  return packedFiles(source).map(relative => ({ relative, bytes: fs.readFileSync(path.join(source, relative)) }));
}
function revisionOf(files) {
  const hash = createHash("sha256");
  for (const { relative, bytes } of files) {
    const name = Buffer.from(relative);
    const length = n => { const out = Buffer.alloc(8); out.writeBigUInt64BE(BigInt(n)); return out; };
    hash.update(length(name.length)); hash.update(name); hash.update(length(bytes.length)); hash.update(bytes);
  }
  return hash.digest("hex");
}
const realKey = JSON.parse(fs.readFileSync(path.join(source, "manifest.json"), "utf8").replace(/^﻿/, "")).key;
if (idOf(DEV_KEY) !== DEV_ID || idOf(realKey) === DEV_ID) throw new Error("개발용 확장 키와 ID 가 어긋나거나 진짜 확장과 같습니다.");

const [first, second, third, fourth] = process.argv.slice(2);
if (first === "--id") {
  console.log(DEV_ID);
  process.exit(0);
}
if (first === "--spki") {
  console.log(sha256(new X509Certificate(fs.readFileSync(second)).publicKey.export({ type: "spki", format: "der" })).toString("base64"));
  process.exit(0);
}
const port = Number(second);
if (!first || !Number.isInteger(port) || port <= 0 || !fourth) {
  console.error("쓰는 법: node scripts/fake-g2b/server.mjs <pfx> <포트> <pfx 암호> <개발 홈>");
  process.exit(64);
}
// 아래에서 통째로 지우고 새로 뜨는 자리 — 늘 <개발 홈>\extension 이다. 개발 홈이 저장소 뿌리면 원본 extension/ 이라 거절한다.
const copy = path.join(path.resolve(fourth), "extension");
if (path.relative(copy, source) === "") {
  console.error(`개발 홈이 저장소 뿌리입니다: ${fourth}`);
  process.exit(64);
}

const revisionMarker = path.join(path.dirname(copy), "extension-revision.json");
function publishRevision(revision) {
  const temporary = `${revisionMarker}.${process.pid}.tmp`;
  fs.writeFileSync(temporary, JSON.stringify({ revision }));
  fs.renameSync(temporary, revisionMarker);
}
function syncExtension() {
  // 표식은 복사 도중 없어서 개발 호스트도 반쯤 쓴 확장을 새 판으로 알리지 않는다.
  fs.rmSync(revisionMarker, { force: true });
  const files = snapshotExtension();
  const revision = revisionOf(files);
  const present = new Set(files.map(file => file.relative));
  fs.mkdirSync(copy, { recursive: true });
  for (const { relative, bytes } of files) {
    const target = path.join(copy, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    if (relative === "manifest.json") fs.writeFileSync(target, manifestOf(bytes.toString("utf8")));
    else if (relative === "background.js") fs.writeFileSync(target, backgroundOf(bytes, revision));
    else fs.writeFileSync(target, bytes);
  }
  for (const relative of packedFiles(copy)) {
    if (!present.has(relative)) fs.rmSync(path.join(copy, relative));
  }
  publishRevision(revision);
}
function backgroundOf(bytes, revision) {
  const background = bytes.toString("utf8");
  if (background.split(REVISION_TOKEN).length !== 2) throw new Error("background.js의 판 자리표를 읽지 못했습니다.");
  return background.replace(REVISION_TOKEN, revision);
}
syncExtension();
fs.watch(source, { recursive: true }, (_, relative) => {
  if (!relative) return;
  try {
    syncExtension();
    console.log(`확장 복사  ${relative}`);
  } catch (e) {
    console.warn(`확장 복사 실패  ${relative}: ${e.message}`); // 편집기가 쓰는 도중이면 다음 알림에서 다시 뜬다
  }
});

const types = { ".html": "text/html; charset=utf-8", ".json": "application/json; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8" };
createServer({ pfx: fs.readFileSync(first), passphrase: third }, (request, response) => {
  let file;
  try {
    const pathname = decodeURIComponent(new URL(request.url, "https://www.g2b.go.kr").pathname);
    file = path.join(here, pathname === "/" ? "index.html" : pathname);
  } catch {
    return response.writeHead(400).end();
  }
  if (!file.startsWith(here + path.sep) || !types[path.extname(file)] || !fs.statSync(file, { throwIfNoEntry: false })?.isFile())
    return response.writeHead(404).end();
  response.writeHead(200, { "content-type": types[path.extname(file)], "cache-control": "no-store" });
  fs.createReadStream(file).pipe(response);
}).listen(port, "127.0.0.1", () => {
  console.log(`가짜 나라장터  https://127.0.0.1:${port}/ (브라우저에서는 https://www.g2b.go.kr/)`);
  console.log(`확장 복사본    ${copy}  (ID ${DEV_ID})`);
});
