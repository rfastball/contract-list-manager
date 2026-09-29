// MAIN world에서만 실행한다. 선언형 프로필의 필드만 돌려주며 원본 표 전체를 전송하지 않는다.
function pclmReadMapped(profiles) {
  if (location.origin !== "https://www.g2b.go.kr" || window !== window.top)
    throw new Error("나라장터의 최상위 업무 화면에서 실행하세요.");
  if ([...document.querySelectorAll('#__processbarIFrame')].some(e => e.checkVisibility({ checkVisibilityCSS: true })))
    throw new Error("화면을 불러오는 중입니다.");
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
  // 화면 판별은 문서 번호 하나로 한다. 표는 있으면 담고 없으면 건드리지 않는다 — 앱은 수집하지 않은 표를
  // 그대로 두고, 덜 실린 표의 빠진 행은 「행 삭제」 충돌로 사람에게 보인다.
  const rank = { request: 1, notice: 2, contract: 3 };
  const candidates = [], failures = [];
  for (const p of profiles.filter(p => p.id !== "g2b-public-notice-header-v1")) {
    if (p.heading && p.heading !== heading) continue;
    try {
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
      if (!readPath(identity, p.baseField) || !readPath(identity, p.seqField)) continue;
      candidates.push({ profile: p.id, rank: rank[p.entityType], snapshot: { profile: p.id, data: { pointInfo, tables }, scope: "live", rowMatches: {} } });
    } catch (e) { failures.push({ rank: rank[p.entityType], message: e.message }); }
  }
  // 화면의 주인은 가장 아랫단 문서다. 계약 화면은 공고·접수 번호를, 공고 화면은 접수 번호를 참조로 싣는다.
  const top = Math.max(0, ...candidates.map(c => c.rank));
  const blocked = failures.filter(f => f.rank > top).sort((a, b) => b.rank - a.rank)[0];
  if (blocked) throw new Error(blocked.message);
  const owners = candidates.filter(c => c.rank === top);
  if (owners.length > 1) throw new Error("여러 자료가 감지되었습니다. 상세 화면 하나만 열어주세요.");
  if (owners.length === 1) return owners[0].snapshot;
  throw new Error("현재 화면에서 접수·공고·계약 번호를 찾지 못했습니다. 화면 자료는 엑셀로 내보낼 수 있습니다.");
}
