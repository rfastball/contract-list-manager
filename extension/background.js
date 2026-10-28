// 계약 목록과 나라장터 화면 사이의 다리. 수집·저장은 앱의 「지금 보는 화면」 이 한다(ADR-036) — 여기는 상시 포트를
// 쥐고 나라장터 탭을 읽어 보고하고, 앱의 명령(다시 읽기·탭 앞으로·엑셀로 내보내기·확장 관리)을 따르며, 툴바 팝업에
// 연결 상태를 알린다. 수동·조건부 자동 저장은 앱이 맡으며 확장에는 저장 RPC 가 없다.
const origin = "https://www.g2b.go.kr";
const failure = (message, extra = {}) => Object.assign(new Error(message), extra);

async function rpc(method, params) {
  const host = (await chrome.storage.local.get("erpDevelopment")).erpDevelopment ? "kr.rfastball.pclm.erp.dev" : "kr.rfastball.pclm.erp";
  const requestId = crypto.randomUUID();
  let response;
  try {
    response = await chrome.runtime.sendNativeMessage(host, { protocolVersion: 2, requestId, method, params });
  } catch (error) {
    throw failure("계약 목록에 연결하지 못했습니다.", { connection: true, detail: error.message });
  }
  if (response?.protocolVersion !== 2 || response.requestId !== requestId || typeof response.ok !== "boolean")
    throw failure("계약 목록의 응답을 확인하지 못했습니다.", { response: true });
  if (!response.ok) {
    if (!response.error?.code || typeof response.error.message !== "string")
      throw failure("계약 목록의 응답을 확인하지 못했습니다.", { response: true });
    const connection = ["setup_required", "schema_mismatch", "wrong_environment"].includes(response.error.code);
    throw failure(connection ? "계약 목록 준비가 필요합니다. 계약 목록의 「Chrome 확장 상태」 를 확인해 주세요." : response.error.message,
      { code: response.error.code, connection, detail: response.error.message });
  }
  return response.result;
}

function browserName() {
  const brands = (navigator.userAgentData?.brands || []).map(b => b.brand);
  return brands.includes("Microsoft Edge") ? "Edge" : brands.includes("Google Chrome") ? "Chrome" : "기타";
}

// 앱을 새 판으로 바꾸면 앱이 확장 파일을 고쳐 두지만, 브라우저는 다시 불러오기 전까지 옛 판을 돈다.
// 호스트가 알려 준 디스크 판이 다르면 스스로 다시 불러온다. 같은 쌍으로 두 번은 하지 않는다 —
// 다른 폴더에서 로드한 확장이면 다시 불러와도 판이 바뀌지 않아 끝없이 되풀이된다.
// 확장이 쥔 것은 읽어 둔 화면뿐이라(저장 중인 것이 없다) 다시 불러오며 잃는 것이 없다. hello 를 받는 길(mirrorHello)이 부른다.
async function reloadIfStale(hello) {
  const current = chrome.runtime.getManifest().version;
  if (typeof hello?.extensionVersion !== "string" || hello.extensionVersion === current) return;
  const attempt = `${current}→${hello.extensionVersion}`;
  if ((await chrome.storage.local.get("reloadAttempt")).reloadAttempt === attempt) return;
  await chrome.storage.local.set({ reloadAttempt: attempt });
  setTimeout(() => chrome.runtime.reload(), 300); // 응답이 나갈 틈을 준다.
  throw failure(`확장을 새 판(${hello.extensionVersion})으로 다시 불러옵니다. 잠시 뒤 화면을 다시 확인해 주세요.`);
}

// 상시 연결(ADR-035). 포트가 열려 있는 동안 브라우저는 워커를 살려 두고 호스트는 포트만큼 산다 — 앱은 호스트가
// 남긴 프로세스별 파일로 확장이 지금 살아 있는지 안다. 인사만 하는 포트라 매핑을 받는 hello 는 여전히 rpc() 로 간다.
// 호스트는 제 exe 가 비켜지면(새 판을 놓으려고 옛 exe 의 이름을 바꿨다) 스스로 떠난다 — 그러면 물러서며 다시 이어
// 그 자리의 새 exe 에 붙는다.
const presenceDelays = [1000, 2000, 5000, 15000, 60000];
let presence = null, presenceOpening = false, presenceTimer = null;
let presenceRetry = 0, presenceSince = 0, presenceGeneration = 0;
function retryPresence() {
  if (presenceTimer) return;
  const delay = presenceDelays[Math.min(presenceRetry++, presenceDelays.length - 1)];
  presenceTimer = setTimeout(() => { presenceTimer = null; connectPresence().catch(() => retryPresence()); }, delay);
}
async function connectPresence() {
  if (presence || presenceOpening) return;
  clearTimeout(presenceTimer); presenceTimer = null;
  presenceOpening = true;
  const generation = presenceGeneration;
  let host;
  try { host = (await chrome.storage.local.get("erpDevelopment")).erpDevelopment ? "kr.rfastball.pclm.erp.dev" : "kr.rfastball.pclm.erp"; }
  catch { return retryPresence(); }
  finally { presenceOpening = false; }
  // 호스트를 고르는 사이 개발 전환이 바뀌었으면 고른 것을 버리고 다시 고른다.
  if (generation !== presenceGeneration) return connectPresence();
  if (presence) return;
  const port = chrome.runtime.connectNative(host);
  presence = port; presenceSince = Date.now();
  // 응답은 쓰지 않는다 — 포트가 살아 있다는 것만이 신호다. 앱이 밀어 보내는 명령(ADR-036)만 받는다.
  port.onMessage.addListener(message => { if (presence === port) mirrorCommand(message); });
  port.onDisconnect.addListener(() => {
    void chrome.runtime.lastError; // 읽지 않으면 브라우저가 확인하지 않은 오류로 남긴다.
    if (presence !== port) return; // 일부러 끊은 옛 포트.
    presence = null;
    stopMirror();
    if (Date.now() - presenceSince >= 60000) presenceRetry = 0;
    retryPresence();
  });
  port.postMessage({ protocolVersion: 2, requestId: crypto.randomUUID(), method: "presence",
    params: { extensionVersion: chrome.runtime.getManifest().version, browser: browserName() } });
  startMirror().catch(() => {});
}
function switchPresence() {
  presenceGeneration++;
  const port = presence;
  presence = null; presenceRetry = 0;
  mirror.hello = null; // 옛 호스트의 작업자료·매핑이다.
  stopMirror();
  clearTimeout(presenceTimer); presenceTimer = null;
  try { port?.disconnect(); } catch { /* 이미 끊겼다 */ }
  connectPresence().catch(() => retryPresence());
}
chrome.runtime.onStartup.addListener(() => { connectPresence().catch(() => retryPresence()); });
connectPresence().catch(() => retryPresence());

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && changes.erpDevelopment) switchPresence();
});

// ── 지금 보는 화면(ADR-036) ─────────────────────────────────
// 상시 포트가 열려 있는 동안만 나라장터 탭을 알고, 읽고, 앱에 보고한다 — 포트가 없으면(앱 호스트 없음) 아무것도 하지
// 않는다. 호스트는 받은 보고를 홈의 파일에 적고 창이 그것을 읽는다. 창의 명령(그 탭을 앞으로·다시 읽기·엑셀로 내보내기)은
// 같은 포트로 돌아온다. 확장은 읽기만 하고 저장하지 않는다 — 저장은 창의 「가져오기」 다.
// 읽는 때: 탭이 앞으로 올 때, 로드가 끝날 때, 창이 부를 때, 그리고 앞 탭은 가벼운 지문을 2초마다 보아 바뀌었을 때
// (WebSquare 는 주소 없이 화면을 바꾼다). 뒤의 탭은 처음 한 번과 부를 때만 읽는다.
const mirrorLoading = "화면을 불러오는 중입니다.";
const mirror = { tabs: new Map(), front: null, frontAt: 0, timer: null, poll: null, sent: "", hello: null, helloAt: 0,
  prints: new Map(), reading: new Set(), again: new Set(), retries: new Map(), generations: new Map(), settings: null };
const isG2b = url => typeof url === "string" && url.startsWith(origin + "/");
const two = n => String(n).padStart(2, "0");
function localStamp() {
  const d = new Date();
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}T${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}
const byteLength = text => new TextEncoder().encode(text).length;

// 수집 규칙(매핑)은 작업자료에 있어 hello 로만 받는다. 읽을 때마다 호스트를 띄우지 않게 잠깐 쥐고 쓴다.
async function mirrorHello() {
  if (mirror.hello && Date.now() - mirror.helloAt < 60000) return mirror.hello;
  const hello = await rpc("hello", { extensionVersion: chrome.runtime.getManifest().version, browser: browserName() });
  await reloadIfStale(hello);
  mirror.hello = hello; mirror.helloAt = Date.now();
  return hello;
}

function trackTab(tab) {
  if (!tab?.id || !isG2b(tab.url)) return false;
  const known = mirror.tabs.get(tab.id);
  mirror.tabs.set(tab.id, { ...(known || { state: "reading", readAt: "", screen: "", menu: "" }), tabId: tab.id, windowId: tab.windowId, title: tab.title || "" });
  if (!known || known.title !== (tab.title || "")) report();
  return !known;
}
function forgetTab(tabId) {
  mirror.generations.set(tabId, (mirror.generations.get(tabId) || 0) + 1);
  mirror.prints.delete(tabId); mirror.retries.delete(tabId);
  if (!mirror.tabs.delete(tabId)) return;
  if (mirror.front === tabId) { mirror.front = null; mirror.frontAt = Date.now(); }
  report();
}

async function readTab(tabId) {
  if (!presence || !mirror.tabs.has(tabId)) return;
  if (mirror.reading.has(tabId)) { mirror.again.add(tabId); return; }
  mirror.reading.add(tabId);
  const generation = mirror.generations.get(tabId) || 0;
  mirror.tabs.set(tabId, { ...mirror.tabs.get(tabId), state: "reading" });
  report();
  let next, retry = false;
  try {
    const hello = await mirrorHello();
    if (generation !== (mirror.generations.get(tabId) || 0)) throw failure("화면이 바뀌어 다시 읽어야 합니다.");
    const injected = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", files: ["capture.js", "export.js"] });
    const documentId = injected[0]?.documentId;
    const [result] = await chrome.scripting.executeScript({ target: { tabId, ...(documentId ? { documentIds: [documentId] } : {}) }, world: "MAIN", args: [hello.mapping.profiles], func: profiles => {
      const screen = document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")?.textContent.trim() || "";
      // 메뉴 번호(ADR-037) — 창이 「수집 안 함」 곁에 보인다. 모르면 빈 글.
      let menu = ""; try { menu = pclmScreenCode(); } catch { /* 모른다 */ }
      // 화면의 이름표와 보이는 글(「화면 그대로」). 읽지 못해도 스냅샷은 그대로 낸다.
      const rows = (profile, snapshot) => { try { return pclmScreenRows(profile, snapshot); } catch { return []; } };
      try {
        const observed = pclmObserveMapped(profiles);
        if (!observed.snapshot) return { screen, menu, ...observed, screenRows: rows(null) };
        return { screen, menu, ...observed, screenRows: rows(profiles.find(p => p.id === observed.snapshot.profile), observed.snapshot) };
      }
      catch (error) { return { screen, menu, error: error.message, unsupported: Boolean(error.unsupported), screenRows: rows(null) }; }
    } });
    const read = result?.result || {};
    if (documentId && result?.documentId !== documentId) throw failure("화면이 바뀌어 다시 읽어야 합니다.");
    if (typeof read.print === "string") mirror.prints.set(tabId, read.print);
    const menu = typeof read.menu === "string" && /^\d{5}$/.test(read.menu) ? read.menu : "";
    const screenRows = Array.isArray(read.screenRows) ? read.screenRows.slice(0, 400) : [];
    if (read.snapshot) {
      const snapshot = { ...read.snapshot, mappingRevision: hello.mappingRevision };
      next = byteLength(JSON.stringify(snapshot)) > (hello.maxRequestBytes || 768 * 1024) - 4096
        ? { state: "error", screen: read.screen, menu, message: "화면 자료가 커서 미리 볼 수 없습니다." }
        : { state: "supported", screen: read.screen, menu, snapshot, screenRows, readiness: read.readiness, datasetId: hello.datasetId };
    } else if (read.error === mirrorLoading && (mirror.retries.get(tabId) || 0) < 5) retry = true;
    else if (read.unsupported) next = { state: "unsupported", screen: read.screen, menu, screenRows };
    else next = { state: "error", screen: read.screen || "", menu, message: read.error || "화면을 읽지 못했습니다." };
  } catch (error) { next = { state: "error", screen: "", menu: "", message: error.message }; }
  finally { mirror.reading.delete(tabId); }
  if (generation !== (mirror.generations.get(tabId) || 0)) {
    if (mirror.again.delete(tabId)) await readTab(tabId);
    return;
  }
  const entry = mirror.tabs.get(tabId);
  if (!entry) return;
  if (retry) {
    // 화면이 아직 그려지는 중이다. 조금 뒤 다시 본다.
    mirror.retries.set(tabId, (mirror.retries.get(tabId) || 0) + 1);
    setTimeout(() => { readTab(tabId).catch(() => {}); }, 1000);
    return;
  }
  mirror.retries.delete(tabId);
  mirror.tabs.set(tabId, { tabId, windowId: entry.windowId, title: entry.title, readAt: localStamp(), ...next, ...(entry.export ? { export: entry.export } : {}) });
  report();
  if (mirror.again.delete(tabId)) await readTab(tabId);
}

// Chrome 에서 앞에 있는 나라장터 탭 — 마지막으로 초점을 가진 창의 활성 탭이 나라장터일 때. 사람이 앱 창으로 옮겨 가
// Chrome 이 초점을 잃어도 그 창의 활성 탭은 그대로 앞이다.
async function refreshFront() {
  let tab;
  try { [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true }); } catch { return; }
  const front = tab?.id && isG2b(tab.url) ? tab.id : null;
  if (front !== null) trackTab(tab);
  if (front === mirror.front) return;
  mirror.front = front; mirror.frontAt = Date.now();
  report();
  if (front !== null) await readTab(front);
}

// 앞 탭의 매핑 원값·표·탭 상태 지문. 자동 관찰은 클릭·스크롤·초점 이동 없이 읽기만 한다.
async function watchFront() {
  const tabId = mirror.front;
  if (!presence || tabId === null || mirror.reading.has(tabId)) return;
  const [result] = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", func: () => {
    const observed = pclmObserveMapped();
    return { print: observed.print, readiness: observed.readiness };
  } });
  const print = result?.result?.print;
  if (typeof print !== "string") return;
  const seen = mirror.prints.get(tabId);
  mirror.prints.set(tabId, print);
  const entry = mirror.tabs.get(tabId);
  if (seen !== undefined && seen !== print || entry?.readiness?.stable !== result.result.readiness?.stable || Date.now() - mirror.helloAt >= 60000) await readTab(tabId);
  else if (entry && result.result.readiness) {
    mirror.tabs.set(tabId, { ...entry, readiness: result.result.readiness });
    report();
  }
}

// 보고는 바뀔 때만, 너무 잦지 않게 묶어 보낸다. 요청 상한을 넘으면 뒤 탭의 스냅샷부터 덜어 낸다 — 앞으로 오면 다시 읽는다.
function report() {
  if (!presence || mirror.timer) return;
  mirror.timer = setTimeout(() => { mirror.timer = null; sendReport(); }, 300);
}
function sendReport() {
  if (!presence) return;
  const limit = (mirror.hello?.maxRequestBytes || 768 * 1024) - 4096;
  let tabs = [...mirror.tabs.values()];
  const params = () => ({ front: mirror.front, frontAt: mirror.frontAt, tabs, settings: mirror.settings });
  let text = JSON.stringify(params());
  for (const victim of tabs.filter(t => t.snapshot && t.tabId !== mirror.front)) {
    if (byteLength(text) <= limit) break;
    tabs = tabs.map(t => t === victim ? { tabId: t.tabId, windowId: t.windowId, title: t.title, readAt: t.readAt, screen: t.screen, menu: t.menu,
      state: "error", message: "열린 탭이 많아 이 화면은 앞으로 가져오면 다시 읽습니다.", ...(t.export ? { export: t.export } : {}) } : t);
    text = JSON.stringify(params());
  }
  if (byteLength(text) > limit) tabs = tabs.map(t => t.snapshot || t.screenRows ? { ...t, snapshot: undefined, screenRows: undefined,
    state: "error", message: "화면 자료가 커서 미리 볼 수 없습니다." } : t);
  text = JSON.stringify(params());
  if (text === mirror.sent) return;
  mirror.sent = text;
  try { presence.postMessage({ protocolVersion: 2, requestId: crypto.randomUUID(), method: "tabs", params: params() }); }
  catch { mirror.sent = ""; }
}

// 확장 설정(「Chrome 확장 상태」 의 설정) — 나라장터 사이트 접근 하나. 사람이 브라우저에서 바꿔도 알림이 늦어
// 앞 탭을 볼 때마다 다시 본다. 바뀌면 다시 보고한다.
async function readSettings() {
  const siteAccess = chrome.permissions?.contains
    ? await chrome.permissions.contains({ origins: [origin + "/*"] }).catch(() => false) : false;
  return { siteAccess: Boolean(siteAccess) };
}
async function refreshSettings() {
  if (!presence) return;
  const next = await readSettings();
  if (JSON.stringify(next) === JSON.stringify(mirror.settings)) return;
  mirror.settings = next;
  report();
}

async function startMirror() {
  mirror.sent = ""; // 새 호스트다 — 같은 것이라도 다시 보낸다.
  mirror.settings = await readSettings().catch(() => null);
  let tabs = [];
  try { tabs = await chrome.tabs.query({ url: origin + "/*" }); } catch { /* 모른다 */ }
  for (const tab of tabs) trackTab(tab);
  await refreshFront();
  // 처음 한 번만 읽는다. 다시 이은 것이면 읽어 둔 것을 그대로 보낸다.
  for (const [tabId, entry] of mirror.tabs) if (!entry.readAt && tabId !== mirror.front) readTab(tabId).catch(() => {});
  report();
  clearInterval(mirror.poll);
  mirror.poll = setInterval(() => { watchFront().catch(() => {}); refreshSettings().catch(() => {}); }, 2000);
}
function stopMirror() {
  clearInterval(mirror.poll); mirror.poll = null;
  clearTimeout(mirror.timer); mirror.timer = null;
}

// 앱이 보낸 명령. 탭 명령은 확장이 아는 나라장터 탭에만 닿는다.
function mirrorCommand(message) {
  if (message?.protocolVersion !== 2 || typeof message.command !== "string") return;
  // 브라우저 안의 확장 관리 — 명령줄로는 chrome://·edge:// 주소를 열 수 없어 창이 확장에 맡긴다. 새 창으로 연다: 브라우저는
  // 앱 창 뒤에 있어 스스로 앞으로 오지 못하고(focused 를 줘도), 앞으로 가져오는 것은 앱이 한다 — 앱은 새로 선 창으로 그것을
  // 알아본다. 탭으로 열면 마지막에 본 창(최소화돼 있을 수도 있다)에 섞여 앱이 가려낼 수 없다.
  if (message.command === "openExtensions") {
    chrome.windows.create({ url: `${browserName() === "Edge" ? "edge" : "chrome"}://extensions/`, focused: true }).catch(() => {});
    return;
  }
  if (!Number.isInteger(message.tabId)) return;
  const entry = mirror.tabs.get(message.tabId);
  if (!entry) return;
  if (message.command === "read") { mirror.prints.delete(message.tabId); readTab(message.tabId).catch(() => {}); }
  else if (message.command === "focusTab")
    chrome.tabs.update(message.tabId, { active: true })
      .then(() => chrome.windows?.update(entry.windowId, { focused: true })).catch(() => {});
  else if (message.command === "export") {
    // 결과는 그 탭의 보고에 실어 앱에 돌려보낸다 — 앱은 요청 뒤에 온 결과로 성공·실패를 말한다.
    const tabId = message.tabId;
    downloadTab(tabId).then(() => ({ ok: true, message: "" }),
      error => ({ ok: false, message: error?.message || "화면을 엑셀로 내보내지 못했습니다." }))
      .then(result => {
        const now = mirror.tabs.get(tabId);
        if (!now) return;
        mirror.tabs.set(tabId, { ...now, export: { at: Date.now(), ...result } });
        report();
      });
  }
}

// 현재 화면 엑셀로 내보내기를 그 탭 안에서 끝낸다 — 앱이 부른 것이라 팝업이 없다. 페이지에 받을 쪽(콘텐츠 스크립트)을
// 두지 않으므로 export.js 를 넣고 같은 자리에서 파일을 지어 내려받기를 건다. 화면이 아직 읽는 중이거나 표가 없어
// 짓지 못하면 그 까닭을 돌려받아 던진다(팝업의 exportActive 와 같은 꼴) — 부른 쪽이 탭 보고로 앱에 알린다.
// 파일 이름 규칙은 팝업의 내려받기(popup.js)와 같다.
async function downloadTab(tabId) {
  await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", files: ["export.js"] });
  const [done] = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", func: () => {
    try {
      const result = pclmExportPage();
      const url = URL.createObjectURL(new Blob([pclmWorkbook(result.sheets)], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" }));
      const link = document.createElement("a");
      link.href = url;
      link.download = `${result.title.replace(/[<>:"/\\|?*\x00-\x1f]/g, "_").slice(0, 80)}_${new Date().toISOString().slice(0, 10)}.xlsx`;
      document.body.append(link); link.click(); link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 30000);
      return { ok: true };
    } catch (error) { return { error: error.message }; }
  } });
  if (!done?.result?.ok) throw failure(done?.result?.error || "화면을 읽지 못했습니다.");
}

// 툴바 팝업의 내보내기. 앱 없이도 된다 — 상시 포트도 hello 도 거치지 않는다. 표만 읽어 돌려주고, 파일은 팝업이 짓는다.
async function exportActive() {
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (!tab?.id || !isG2b(tab.url)) throw failure("나라장터 업무 화면을 열어 주세요.");
  await chrome.scripting.executeScript({ target: { tabId: tab.id }, world: "MAIN", files: ["export.js"] });
  const [result] = await chrome.scripting.executeScript({ target: { tabId: tab.id }, world: "MAIN", func: () => {
    try { return { workbook: pclmExportPage() }; }
    catch (error) { return { error: error.message }; }
  } });
  if (!result?.result?.workbook) throw failure(result?.result?.error || "화면을 읽지 못했습니다.");
  return { kind: "exported", ...result.result.workbook };
}

// 툴바 팝업이 그리는 연결 상태. 연결은 상시 포트가 살아 있는지, 이 탭은 거울이 읽어 둔 것이다 — 팝업을 열었다고 새로
// 읽지 않는다. 작업자료 이름은 hello 가 알려 준 것이고, 연결돼 있는데 아직 모르면 한 번 묻는다.
async function popupStatus() {
  let tab;
  try { [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true }); } catch { /* 모른다 */ }
  const g2b = Boolean(tab?.id && isG2b(tab.url));
  const known = g2b ? mirror.tabs.get(tab.id) : undefined;
  if (presence && !mirror.hello) await mirrorHello().catch(() => {});
  const development = (await chrome.storage.local.get("erpDevelopment").catch(() => ({}))).erpDevelopment;
  const hello = presence ? mirror.hello : null;
  return { connected: Boolean(presence),
    destination: typeof hello?.destination === "string" ? hello.destination : undefined,
    environment: typeof hello?.environment === "string" ? hello.environment : development ? "development" : undefined,
    tab: known ? { g2b, state: known.state, screen: known.screen || "", kind: known.snapshot?.profile, message: known.message } : { g2b } };
}

// 팝업만 묻는다 — 탭(페이지)에서 온 메시지는 받지 않는다.
chrome.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== chrome.runtime.id || sender.tab) return;
  if (message?.type === "pclm-status") { popupStatus().then(respond, () => respond({ connected: false, tab: { g2b: false } })); return true; }
  if (message?.type === "pclm-export") {
    exportActive().then(respond, error => respond({ kind: "blocked", message: error.message }));
    return true;
  }
});

chrome.tabs.onActivated?.addListener(() => { if (presence) refreshFront().catch(() => {}); });
chrome.tabs.onUpdated?.addListener((tabId, change, tab) => {
  if (!presence) return;
  if (!isG2b(tab?.url)) {
    if (mirror.tabs.has(tabId)) { forgetTab(tabId); if (tab?.active) refreshFront().catch(() => {}); }
    return;
  }
  trackTab(tab);
  if (change.status === "loading" || change.url) {
    mirror.generations.set(tabId, (mirror.generations.get(tabId) || 0) + 1);
    mirror.tabs.set(tabId, { ...mirror.tabs.get(tabId), state: "reading", readiness: undefined });
    report();
  }
  if (change.status === "complete") { mirror.prints.delete(tabId); readTab(tabId).catch(() => {}); }
  if (tab.active && mirror.front !== tabId) refreshFront().catch(() => {});
});
chrome.tabs.onRemoved.addListener(tabId => forgetTab(tabId));
chrome.permissions?.onAdded?.addListener(() => { refreshSettings().catch(() => {}); });
chrome.permissions?.onRemoved?.addListener(() => { refreshSettings().catch(() => {}); });
chrome.windows?.onFocusChanged?.addListener(windowId => {
  if (presence && windowId !== chrome.windows.WINDOW_ID_NONE) refreshFront().catch(() => {});
});
