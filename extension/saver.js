// 문서 파일 저장. 폴더를 고르지 않았으면 다운로드\계약문서 에 내려받고(chrome.downloads, ADR-046), 고른 폴더가 있으면 그
// 폴더에 쓴다(File System Access — 나르미 컴팩트 0.2.0 의 것을 옮겼다, ADR-045). 패널에서만 쓴다 — 고르기 창과 권한 물음은
// 사람 동작 안에서만 뜬다. 파일 쓰기는 exe 를 거치지 않는다.
//
// spike(설계 저장소 spikes/side-panel)에서 확인한 것:
//  - 문서·바탕화면·다운로드 폴더 자체와 드라이브 뿌리(C:\)는 Chrome 이 막는다. 그 안의 하위 폴더는 된다.
//  - 패널을 닫았다 열면 권한이 「물음」으로 돌아간다 → 패널을 연 뒤 첫 저장에서 한 번 묻는다.
//  - 덮어쓰기 실패: 한글에서 연 파일은 InvalidStateError, 다른 프로그램이 쥔 파일은 NotReadableError. 어느 쪽이든 이전 파일은 그대로다.
//
// 「우리가 쓴 파일」은 saved 저장소에 SHA-256 을 적어 둔다. 같은 이름이 있으면 지문을 비교해
// 우리가 쓴 그대로인지, 사람이 고친 파일인지 나눠 묻는다.

import * as store from "./store.js";

export const PICKER = { id: "pclm-document-folder", mode: "readwrite", startIn: "documents" };
const RW = { mode: "readwrite" };

export async function sha256(bytes) {
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return [...hash].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export const folder = () => store.setting("folder");

// 고르지 않았을 때의 저장 자리 — 다운로드 폴더 안의 이 폴더.
export const DEFAULT_DIR = "계약문서";
export const DEFAULT_LABEL = `다운로드\\${DEFAULT_DIR}`;

const baseName = (path) => String(path).split(/[\\/]/).pop();

/**
 * 내려받기를 걸고 끝날 때까지 기다린다 → {id, name}(번호가 붙었을 수 있는 실제 파일 이름) | null(사람이 저장 창을 취소).
 * 다른 중단은 이름이 Chrome 의 사유 코드(FILE_NO_SPACE 등)인 오류로 던진다. downloads 는 chrome.downloads(시험은 가짜).
 */
export async function waitDownload(downloads, options) {
  const id = await downloads.download(options);
  return new Promise((resolve, reject) => {
    let settled = false;
    const settle = async (state) => {
      if (settled || !["complete", "interrupted"].includes(state)) return;
      settled = true;
      downloads.onChanged.removeListener(listen);
      try {
        const [item] = await downloads.search({ id });
        if (state === "complete") return resolve({ id, name: baseName(item?.filename || options.filename) });
        const reason = item?.error || "FILE_FAILED";
        if (reason === "USER_CANCELED") return resolve(null);
        reject(Object.assign(new Error(reason), { name: reason }));
      } catch (error) {
        reject(error);
      }
    };
    const listen = (delta) => { if (delta.id === id) settle(delta.state?.current); };
    downloads.onChanged.addListener(listen);
    // 듣기 전에 이미 끝났을 수 있다(작은 파일).
    downloads.search({ id }).then(([item]) => settle(item?.state), reject);
  });
}

/**
 * 기본 자리(다운로드\계약문서)에 저장. 같은 이름이 있으면 Chrome 이 「이름 (1)」 처럼 번호를 붙인다 — 덮어쓰지 않으므로
 * 고른 폴더 길의 같은 이름 확인(ours · edited)이 없다. 돌려주는 것: {record, id} | null(취소).
 */
export async function download(name, bytes, meta) {
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
  try {
    const done = await waitDownload(chrome.downloads, { url, filename: `${DEFAULT_DIR}/${name}`, conflictAction: "uniquify", saveAs: false });
    if (!done) return null;
    const record = { name: done.name, sha256: await sha256(bytes), savedAt: Date.now(), ...meta, downloadId: done.id };
    await store.put("saved", done.name, record);
    return { record, id: done.id };
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** 폴더 고르기 창. 취소하면 null. */
export async function chooseFolder() {
  let handle;
  try {
    handle = await window.showDirectoryPicker(PICKER);
  } catch (error) {
    if (error.name === "AbortError") return null;
    throw new Error(explain(error));
  }
  await store.setSetting("folder", handle);
  return handle;
}

/** 쓸 권한. 사람 동작 안에서 불러야 물음이 뜬다(저장 단추를 누른 곧바로). */
export async function permit(handle) {
  if ((await handle.queryPermission?.(RW)) === "granted") return true;
  return (await handle.requestPermission?.(RW)) === "granted";
}

/** 같은 이름의 파일을 살핀다 → null | {kind: "ours"|"edited"|"unknown", record, lastModified} */
export async function existing(dir, name) {
  let file;
  try {
    file = await (await dir.getFileHandle(name)).getFile();
  } catch (error) {
    if (error.name === "NotFoundError" || error.name === "TypeMismatchError") return null;
    throw error;
  }
  const record = await store.get("saved", name);
  const hash = await sha256(new Uint8Array(await file.arrayBuffer()));
  const kind = !record ? "unknown" : record.sha256 === hash ? "ours" : "edited";
  return { kind, record, lastModified: file.lastModified };
}

/** 없어진 것이 확인된 파일만 경고에서 뺀다. 권한·기록 조회 실패는 파일 삭제의 증거가 아니다. 저장 기록은 보존한다. */
export async function savedFiles(records, downloads = chrome.downloads, extensionId = chrome.runtime.id) {
  const present = await Promise.all(Object.entries(records).map(async ([name, record]) => {
    try {
      if (record.fileHandle) {
        try { await record.fileHandle.getFile(); }
        catch (error) { if (error.name === "NotFoundError") return null; throw error; }
      } else {
        let item;
        if (record.downloadId != null) [item] = await downloads.search({ id: record.downloadId });
        else {
          // 이전 기록에는 위치가 없다. 이 확장이 같은 시각에 내려받은 파일만 원래 파일로 인정한다.
          const escaped = `${DEFAULT_DIR}/${name}`.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replaceAll("/", "[\\\\/]");
          const matches = (await downloads.search({ filenameRegex: `(?:^|[\\\\/])${escaped}$` })).filter((d) =>
            d.byExtensionId === extensionId && d.state === "complete"
            && record.savedAt >= Date.parse(d.endTime) && record.savedAt - Date.parse(d.endTime) < 10_000);
          if (matches.length === 1) [item] = matches;
        }
        if (item?.exists === false) return null;
      }
    } catch { /* 확인할 수 없으면 경고와 추적 기록을 유지한다. */ }
    return [name, record];
  }));
  return Object.fromEntries(present.filter(Boolean));
}

/** 이 폴더에 아직 없는 이름: 「이름 (2).hwpx」, 「이름 (3).hwpx」… */
export async function freeName(dir, name) {
  const stem = name.replace(/\.hwpx$/i, "");
  for (let i = 2; i < 100; i++) {
    const candidate = `${stem} (${i}).hwpx`;
    try {
      await dir.getFileHandle(candidate);
    } catch (error) {
      if (error.name === "NotFoundError") return candidate;
      throw error;
    }
  }
  throw new Error("같은 이름의 파일이 너무 많습니다. 폴더를 정리한 뒤 다시 저장하세요.");
}

/** 쓴다. 쓰는 중에 실패하면 이전 파일은 그대로다(Chrome 이 임시 파일에 쓰고 닫을 때 바꾼다). */
export async function write(dir, name, bytes, meta) {
  const handle = await dir.getFileHandle(name, { create: true });
  const writable = await handle.createWritable();
  try {
    await writable.write(bytes);
    await writable.close();
  } catch (error) {
    await writable.abort().catch(() => {});
    throw error;
  }
  const record = { name, sha256: await sha256(bytes), savedAt: Date.now(), ...meta, fileHandle: handle };
  await store.put("saved", name, record);
  return record;
}

/** 실패를 사람 말로. */
export function explain(error) {
  switch (error?.name) {
    case "InvalidStateError":
      return "한글에서 이 파일이 열려 있어 바꾸지 못했습니다. 한글에서 파일을 닫은 뒤 다시 저장하세요. 이전 파일은 그대로입니다.";
    case "NotReadableError":
      return "다른 프로그램이 이 파일을 쓰고 있어 바꾸지 못했습니다. 그 프로그램을 닫은 뒤 다시 저장하세요. 이전 파일은 그대로입니다.";
    case "NotAllowedError":
    case "SecurityError":
      return "폴더에 쓸 권한을 받지 못했습니다. 저장을 다시 누르고 Chrome 이 물으면 「수정 허용」을 고르세요.";
    case "NotFoundError":
      return "저장 폴더를 찾지 못했습니다. 폴더를 옮기거나 지웠다면 「설정」에서 다시 고르세요.";
    case "QuotaExceededError":
    case "FILE_NO_SPACE":
      return "디스크에 자리가 없어 저장하지 못했습니다.";
    case "FILE_ACCESS_DENIED":
      return "다운로드 폴더에 쓰지 못했습니다. 「설정」에서 다른 폴더를 고르세요.";
    default:
      return `저장하지 못했습니다: ${error?.message || error}`;
  }
}
