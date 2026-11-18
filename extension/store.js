// 문서 만들기 패널의 IndexedDB(ADR-045 덧붙여 정한 것) — 「내 정보」 와 패널 설정은 이 확장에 둔다. 작업자료 · 홈에 두지 않는다.
// 서식은 여기 두지 않는다 — 서식 폴더의 파일이고 호스트가 읽어 넘긴다(ADR-049). 서식을 가리키는 열쇠는 모두 그 파일 이름이다.
// prepared 는 그 파일의 컴파일본일 뿐이라 지워도 다음 읽기에서 다시 선다.
//
//   saved     열쇠 파일 이름 → {name, sha256, caseKeys, templateId(서식 파일 이름), savedAt, fileHandle|downloadId}  파일의 지문·위치
//   settings  열쇠 이름      → 값(me, folder 핸들, lastTemplate, fileNamePatterns {파일 이름: 패턴}, selections {파일 이름: {항목: 선택}},
//                              templateOrder [파일 이름], hiddenTemplates [파일 이름])
//   prepared  열쇠 서식 파일 이름 → {source(원본 바이트 SHA-256), engine(engine.wasm SHA-256), bytes, fields, slots}
//                              한글 서식의 컴파일본(templates.js loadTemplates). source · engine 이 같을 때만 쓴다

const NAME = "pclm-document";
const VERSION = 2;
const STORES = ["saved", "settings", "prepared"];

let opening;
function db() {
  opening ||= new Promise((resolve, reject) => {
    const req = indexedDB.open(NAME, VERSION);
    req.onupgradeneeded = () => {
      for (const s of STORES) if (!req.result.objectStoreNames.contains(s)) req.result.createObjectStore(s);
    };
    req.onsuccess = () => {
      req.result.onversionchange = () => { req.result.close(); opening = null; };
      resolve(req.result);
    };
    req.onerror = () => { opening = null; reject(req.error); };
  });
  return opening;
}

async function run(store, mode, fn) {
  const tx = (await db()).transaction(store, mode);
  const req = fn(tx.objectStore(store));
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve(req?.result);
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

export const get = (store, key) => run(store, "readonly", (s) => s.get(key));
export const put = (store, key, value) => run(store, "readwrite", (s) => s.put(value, key));
export const remove = (store, key) => run(store, "readwrite", (s) => s.delete(key));

/** 저장소 전체를 {열쇠: 값} 으로. */
export async function all(store) {
  const tx = (await db()).transaction(store, "readonly");
  const s = tx.objectStore(store);
  const [keys, values] = await Promise.all([s.getAllKeys(), s.getAll()].map((req) => new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  })));
  return Object.fromEntries(keys.map((k, i) => [k, values[i]]));
}

export const setting = (key) => get("settings", key);
export const setSetting = (key, value) => put("settings", key, value);
