// 지금 화면의 메뉴 번호(다섯 자리) — 나라장터가 스스로 쓰는 함수로 읽고, 없으면 주소 상태에서. 모르면 "".
function pclmScreenCode() {
  const valid = v => (typeof v === "string" || typeof v === "number") && /^\d{5}$/.test(String(v)) ? String(v) : "";
  let code = "";
  try { code = valid(window.com?.gfnGetMenuNo?.()); } catch { /* 아래로 */ }
  if (!code) try { code = valid(history.state?.data?.menuNo); } catch { /* 모른다 */ }
  return code;
}

// MAIN world에서만 실행한다. 선언형 프로필의 필드만 돌려주며 원본 표 전체를 전송하지 않는다.
// 수집하는 화면(ADR-037)은 hello 의 매핑에서 screen 을 단 프로필이다 — 그 프로필이 그 화면의 고유키도 들고 있다. 받는지는
// 화면의 메뉴 번호가 그 목록에 있는가로만 정하고, 화면 하나에 프로필 하나다. 번호를 읽지 못했거나 목록에 없거나 목록이 비었으면
// (화면을 싣지 않는 옛 호스트) unsupported 표시를 단 오류를 던진다 — 지금 보는 화면이 그것으로 「수집 안 함」 을 세운다.
// 목록의 화면인데 그 프로필의 번호 칸을 읽지 못하면 그것은 평범한 오류다(다시 읽기).
function pclmReadMapped(profiles) {
  if (location.origin !== "https://www.g2b.go.kr" || window !== window.top)
    throw new Error("나라장터의 최상위 업무 화면에서 실행하세요.");
  if ([...document.querySelectorAll('#__processbarIFrame')].some(e => e.checkVisibility({ checkVisibilityCSS: true })))
    throw new Error("화면을 불러오는 중입니다.");
  const unsupported = message => Object.assign(new Error(message), { unsupported: true });
  const supported = (Array.isArray(profiles) ? profiles : []).filter(p => typeof p?.screen?.code === "string");
  if (!supported.length)
    throw unsupported("앱이 수집하는 화면을 알려 주지 않았습니다. 앱을 새 판으로 바꿔 주세요.");
  const code = pclmScreenCode();
  if (!code) throw unsupported("화면 번호를 읽지 못해 수집하지 않습니다. 화면 자료는 엑셀로 내보낼 수 있습니다.");
  const p = supported.find(p => p.screen.code === code);
  if (!p) throw unsupported(`메뉴 ${code} 화면은 수집하지 않습니다. 화면 자료는 엑셀로 내보낼 수 있습니다.`);
  const heading = document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")?.textContent.trim();
  const component = id => {
    try { return window.$p?.getComponentById?.(id); } catch { return null; }
  };
  // 값은 컴포넌트의 원값(getValue)을 먼저 읽는다 — input.value 는 표시 서식(2026/07/15, 1,000)이라
  // 같은 필드가 원값·서식값 두 가지로 관찰된다. 화면에 안 보이는 칸도 같은 자료에 묶여 있으므로
  // 보이는 칸에 없을 때만 쓴다(차수처럼 번호 옆에 따로 표시하지 않는 값).
  const scalar = (key, selector, strict) => {
    const shown = [], hidden = [];
    for (const e of document.querySelectorAll(selector || "#mf_wfm_container [id]")) {
      const c = component(e.id);
      let ref; try { ref = c?.getRef?.(); } catch { continue; }
      const bound = String(ref || "").split(/[.:/]/).at(-1) === key;
      if (!selector && !bound && e.id !== key && !e.id.endsWith("_" + key)) continue;
      let value;
      try { if (c?.getValue) value = c.getValue(); } catch { /* DOM 값으로 읽는다. */ }
      if (value === undefined || value === null || typeof value === "object") value = undefined;
      if (value === undefined && e.matches("input,textarea,select")) value = e.value;
      else if (value === undefined && selector) value = e.textContent.trim();
      if (value === undefined || value === null) continue;
      (e.checkVisibility({ checkVisibilityCSS: true }) ? shown : hidden).push({ bound, value: String(value) });
    }
    // 자료에 묶인 칸이 ID 이름만 닮은 칸보다 우선한다.
    const pool = shown.length ? shown : hidden;
    const best = pool.some(v => v.bound) ? pool.filter(v => v.bound) : pool;
    const unique = [...new Set(best.map(v => v.value))];
    if (unique.length > 1) {
      if (strict) throw new Error(`필드가 여러 값으로 관찰됩니다: ${key}`);
      return undefined; // 부가 필드 하나 때문에 문서 전체를 막지 않는다. 미수집 필드는 기존 값을 건드리지 않는다.
    }
    return unique[0];
  };
  const readPath = (value, path) => path.split('.').reduce((v, k) => v?.[k], value);
  const pick = (row, paths) => {
    const selected = {};
    for (const path of paths) {
      const value = readPath(row, path); if (value === undefined) continue;
      const parts = path.split('.'); let target = selected;
      for (const part of parts.slice(0, -1)) target = target[part] ||= {};
      target[parts.at(-1)] = value;
    }
    return selected;
  };
  // 화면 판별은 메뉴 번호로 이미 했다. 표는 있으면 담고 없으면 건드리지 않는다 — 앱은 수집하지 않은 표를
  // 그대로 두고, 덜 실린 표의 빠진 행은 「행 삭제」 충돌로 사람에게 보인다.
  const notRead = "화면에서 번호를 읽지 못했습니다. 화면이 다 열린 뒤 다시 읽어 주세요.";
  if (p.heading && p.heading !== heading) throw new Error(notRead);
  const tables = {}, pointInfo = {};
  for (const t of p.tables) {
    const ids = [...document.querySelectorAll('[id]')].map(e => e.id).filter(id => t.source.startsWith('*') ? id.endsWith(t.source.slice(1)) : id === t.source);
    if (ids.length !== 1) continue;
    const grid = component(ids[0]);
    let list = grid?.getDataList?.();
    if (typeof list === "string") list = component(list);
    list ||= grid;
    const raw = list?.getAllJSON?.();
    if (!Array.isArray(raw)) continue;
    tables[ids[0]] = raw.map(row => pick(row, [...t.keys, ...t.fields.flatMap(f => f.secondarySource ? [f.source, f.secondarySource] : [f.source])]));
  }
  for (const key of new Set([...p.fields.flatMap(f => f.secondarySource ? [f.source, f.secondarySource] : [f.source]), ...(p.identitySource === "pointInfo" ? [p.baseField, p.seqField] : [])])) {
    if (key.startsWith("tables.")) {
      const [, id, , field] = key.split('.');
      const grid = component(id); let list = grid?.getDataList?.();
      if (typeof list === "string") list = component(list);
      const raw = (list || grid)?.getAllJSON?.();
      if (Array.isArray(raw) && raw.length === 1) tables[id] = [pick(raw[0], [field])];
      continue;
    }
    const value = scalar(key, p.selectors?.[key], key === p.baseField || key === p.seqField);
    if (value !== undefined) {
      const parts = key.split("."); let target = pointInfo;
      for (const part of parts.slice(0, -1)) target = target[part] ||= {};
      target[parts.at(-1)] = value;
    }
  }
  const identity = p.identitySource === "pointInfo" ? pointInfo : tables[p.identitySource]?.[0];
  if (!readPath(identity, p.baseField) || !readPath(identity, p.seqField)) throw new Error(notRead);
  return { profile: p.id, data: { pointInfo, tables }, scope: "live", rowMatches: {}, screen: code };
}

// 지금 보는 화면(ADR-036)의 「화면 그대로」 — 수집 규칙이 읽는 칸마다 화면에 적힌 이름표와 보이는 글. 값은 화면에 보인
// 그대로다(서식 그대로, 꾸미지 않는다). 품목 표는 줄마다 「순번 품명」 / 「수량단위 · 금액」 의 원값. 수집 규칙에 없는
// 화면(profile 없음)은 내보내기의 화면 필드를 앞에서 40줄까지. export.js 가 함께 들어와 있어야 한다(pclmLabel·pclmExportPage).
// 자료는 수집 규칙이 허용한 칸과 내보내기가 거른 칸만 — 그 밖의 칸을 새로 읽지 않는다.
function pclmScreenRows(profile, snapshot) {
  const text = v => (v === undefined || v === null ? "" : String(v));
  if (!profile) {
    let page;
    try { page = pclmExportPage(); } catch { return []; }
    const fields = page.sheets.find(s => s.source === "DOM");
    return fields ? fields.rows.slice(1, 41).map(([label, value]) => ({ group: "", label: text(label), text: text(value), source: "" })) : [];
  }
  const component = id => { try { return window.$p?.getComponentById?.(id); } catch { return null; } };
  const visible = e => e.checkVisibility({ checkVisibilityCSS: true });
  const header = document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle");
  const headings = [...document.querySelectorAll('h1, h2, h3, h4, h5, h6, [role="heading"]')]
    .filter(h => h !== header && !h.closest("#pclm-collector") && visible(h));
  // 가장 가까운 앞의 구역 제목. 없으면 빈 글 — 묶음 하나로 선다.
  const groupOf = e => {
    let found = "";
    for (const h of headings)
      if (!h.contains(e) && h.compareDocumentPosition(e) & Node.DOCUMENT_POSITION_FOLLOWING) found = h.textContent.trim();
    return found;
  };
  // 이름표 — 칸에 붙은 것, 없으면 같은 표 줄의 앞 th.
  const labelOf = e => {
    const own = pclmLabel(e);
    if (own) return own;
    const cell = e.closest("td, th");
    for (let c = cell?.previousElementSibling; c; c = c.previousElementSibling)
      if (c.tagName === "TH") return c.textContent.trim();
    return "";
  };
  const shown = e => e.matches("select") ? [...e.selectedOptions].map(o => o.textContent.trim()).join(", ")
    : e.matches("input, textarea") ? e.value : e.textContent.trim();
  // 그 칸을 내는 요소 — pclmReadMapped 의 scalar 와 같은 고름(자료에 묶인 것 · 보이는 것 먼저).
  const elementOf = key => {
    const selector = profile.selectors?.[key];
    const found = [];
    for (const e of document.querySelectorAll(selector || "#mf_wfm_container [id]")) {
      let ref; try { ref = component(e.id)?.getRef?.(); } catch { continue; }
      const bound = String(ref || "").split(/[.:/]/).at(-1) === key;
      if (!selector && !bound && e.id !== key && !e.id.endsWith("_" + key)) continue;
      found.push({ e, rank: (visible(e) ? 2 : 0) + (bound ? 1 : 0) });
    }
    return found.sort((a, b) => b.rank - a.rank)[0]?.e;
  };
  const rows = [];
  const keys = [...new Set([...(profile.identitySource === "pointInfo" ? [profile.baseField, profile.seqField] : []),
    ...profile.fields.map(f => f.source)])].filter(key => !key.startsWith("tables."));
  for (const key of keys) {
    const e = elementOf(key);
    if (e) rows.push({ group: groupOf(e), label: labelOf(e), text: shown(e), source: key });
  }
  const table = profile.tables.find(t => t.target === profile.entityType + "_item");
  const id = table && Object.keys(snapshot?.data?.tables || {}).find(k => table.source.startsWith("*") ? k.endsWith(table.source.slice(1)) : k === table.source);
  if (id) {
    const grid = document.getElementById(id);
    const group = (grid && (pclmLabel(grid) || groupOf(grid))) || "";
    const of = target => table.fields.find(f => f.target === target)?.source;
    const [name, quantity, unit, amount, price] = ["item_name", "quantity", "unit", "amount", "unit_price"].map(of);
    const order = table.keys.at(-1);
    snapshot.data.tables[id].forEach((row, i) => rows.push({
      group,
      label: [text(row[order] ?? i + 1), text(name && row[name])].filter(Boolean).join(" "),
      text: [text(quantity && row[quantity]) + text(unit && row[unit]), text((amount && row[amount]) ?? (price && row[price]))].filter(Boolean).join(" · "),
      source: `table:${table.target}:${i + 1}`,
    }));
  }
  return rows;
}
