// 화면이 칸에 붙여 둔 이름표 — aria-label → title → <label> → 가장 가까운 [data-title]. 내보내기와 지금 보는 화면의
// 이름표(capture.js 의 pclmScreenRows)가 같은 것을 쓴다.
function pclmLabel(e) {
  return String(e.getAttribute("aria-label") || e.title ||
    [...(e.labels || [])].map(l => l.textContent).join(" ") || e.closest("[data-title]")?.getAttribute("data-title") || "").trim();
}

// MAIN world 수집. 페이지 이동이나 추가 조회 없이 현재 로딩된 업무 자료만 읽는다.
function pclmExportPage() {
  if (location.origin !== "https://www.g2b.go.kr" || window !== window.top)
    throw new Error("나라장터 업무 화면에서 실행해 주세요.");
  const visible = e => !e.closest('[hidden], [aria-hidden="true"], nav, [role="navigation"], #pclm-collector') && e.checkVisibility({ checkVisibilityCSS: true });
  if ([...document.querySelectorAll('#__processbarIFrame')].some(visible))
    throw new Error("화면을 불러오는 중입니다. 로딩이 끝난 뒤 다시 내보내세요.");
  const clean = value => String(value ?? "").trim();
  const internal = /password|passwd|pwd|token|session|cookie|csrf|account(?!ing)|actno|prsnno|rrno|rrn|atflpath|ipar|비밀번호|주민등록|계좌번호/i;
  const excluded = 'script, style, button, input[type="hidden"], input[type="password"], input[type="button"], input[type="submit"], input[type="reset"], input[type="file"], input[type="image"]';
  const component = id => { try { return window.$p?.getComponentById?.(id); } catch { return null; } };
  const label = e => clean(pclmLabel(e));
  const valueOf = e => {
    if (!e || !visible(e) || e.matches(excluded) || internal.test(e.id + " " + label(e))) return "";
    if (e.matches('input[type="radio"]')) return e.checked ? clean(e.value) : "";
    if (e.matches('input[type="checkbox"]')) return e.checked ? "예" : "아니오";
    if (e.matches("select")) return [...e.selectedOptions].map(option => clean(option.textContent)).join(", ");
    if (e.matches("input,textarea")) return clean(e.value);
    return clean([...e.childNodes].map(node => node.nodeType === 3 ? node.textContent : node.nodeType === 1 ? valueOf(node) : "").join(" "));
  };
  const sheets = [], covered = [], lists = new Set(), warnings = [];
  const add = (name, source, rows) => {
    if (rows.length > 1) sheets.push({ name, source, rows });
  };
  // ponytail: 임의 중첩 객체는 단일 값의 경로만 펼친다. 배열 자료는 별도 표가 검증될 때 지원한다.
  const flatten = (row, prefix = "", result = {}) => {
    if (!row || typeof row !== "object" || Array.isArray(row)) return result;
    for (const [key, value] of Object.entries(row)) {
      const path = prefix + key;
      if (internal.test(path) || key.startsWith("_") || ["rowStatus", "rowType", "rowIndex"].includes(key)) continue;
      if (value && typeof value === "object") { if (!Array.isArray(value)) flatten(value, path + ".", result); }
      else result[path] = clean(value);
    }
    return result;
  };
  for (const e of document.querySelectorAll("[id]")) {
    if (!visible(e)) continue;
    const grid = component(e.id);
    if (!grid?.getDataList) continue;
    covered.push(e); // 원천 표를 못 읽어도 DOM의 일부 행을 전체 표처럼 대체하지 않는다.
    try {
      let list = grid.getDataList();
      if (typeof list === "string") list = component(list);
      if (!list?.getAllJSON) throw new Error("DataList 없음");
      if (lists.has(list)) continue;
      lists.add(list);
      const raw = list.getAllJSON();
      if (!Array.isArray(raw)) throw new Error("행 목록 없음");
      const rows = raw.map(row => flatten(row));
      const keys = [...new Set(rows.flatMap(row => Object.keys(row)))].filter(key => rows.some(row => row[key] !== undefined && row[key] !== ""));
      if (keys.length) add(label(e) || e.id, e.id, [keys, ...rows.map(row => keys.map(key => row[key] ?? ""))]);
    } catch { warnings.push(`표를 읽지 못함: ${e.id}`); }
  }
  // 일반 HTML 표도 머리글이 있는 업무 표만 수집한다. 레이아웃 표는 아래 필드 수집에서 처리한다.
  for (const table of document.querySelectorAll("table[id]")) {
    if (!visible(table) || covered.some(e => e === table || e.contains(table))) continue;
    const header = [...table.rows].find(row => visible(row) && [...row.cells].some(cell => cell.tagName === "TH"));
    if (!header) continue;
    const body = [...table.rows].filter(row => row !== header && visible(row) && row.closest("table") === table);
    if (!body.length || body.some(row => [...row.cells].some(cell => cell.tagName === "TH"))) continue;
    covered.push(table);
    if ([...table.rows].some(row => [...row.cells].some(cell => cell.colSpan > 1 || cell.rowSpan > 1))) {
      warnings.push(`병합 셀이 있는 표는 제외함: ${table.id}`); continue;
    }
    const columns = [...header.cells].map((cell, index) => ({ name: valueOf(cell), index }))
      .filter(col => col.name && !internal.test(col.name) && body.some(row => valueOf(row.cells[col.index])));
    if (columns.length) add(label(table) || clean(table.caption?.textContent) || table.id, table.id,
      [columns.map(col => col.name), ...body.map(row => columns.map(col => valueOf(row.cells[col.index])))]);
  }
  const fields = [["항목", "값", "원본 ID", "참조 필드"]];
  for (const e of document.querySelectorAll('input[id], textarea[id], select[id], .w2textbox[id], [data-title] > span[id]')) {
    if (!visible(e) || covered.some(table => table.contains(e)) || e.matches(excluded)) continue;
    const c = component(e.id);
    let ref = "";
    try { ref = clean(c?.getRef?.()); } catch { /* DOM 값은 읽을 수 있다. */ }
    const name = label(e) || ref.split(/[.:/]/).at(-1) || e.id;
    if (internal.test(e.id + " " + name + " " + ref)) continue;
    const value = valueOf(e);
    if (value) fields.push([name, value, e.id, ref]);
  }
  if (fields.length > 1) sheets.unshift({ name: "화면 필드", source: "DOM", rows: fields });
  if (!sheets.length) throw new Error(warnings[0] || "내보낼 필드나 표가 없습니다. 자료가 있는 업무 화면을 열어 주세요.");
  const title = clean(document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")?.textContent || document.title) || "나라장터";
  const url = location.origin + location.pathname; // 세션 값이 포함될 수 있는 query/hash는 기록하지 않는다.
  sheets.push({ name: "수집 정보", source: "", rows: [["항목", "내용"], ["화면", title], ["출처", url],
    ["수집 시각", new Date().toISOString()], ["범위", "현재 화면의 표시 필드와 로딩된 표 데이터. 다른 페이지·미개방 탭·전체 건수는 확인하지 않음."],
    ...sheets.map(s => [s.name, `${s.source} · ${s.rows.length - 1}행`]), ...warnings.map(w => ["미수집", w])] });
  return { title, sheets, warnings };
}

// 의존성 없이 문자열 셀만 쓰는 OOXML 통합 문서. 번호의 선행 0과 수식처럼 보이는 원문을 보존한다.
function pclmWorkbook(sheets) {
  const xml = value => String(value ?? "").replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\ufffe\uffff]/g, "")
    .replace(/_x[0-9a-f]{4}_/gi, value => "_x005F_" + value.slice(1))
    .replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;" })[c]);
  const ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
  const rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const column = n => { let s = ""; for (n++; n; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + (n - 1) % 26) + s; return s; };
  if (!Array.isArray(sheets) || !sheets.length || sheets.length > 200) throw new Error("내보낼 시트 수를 확인하세요. 최대 200개입니다.");
  const files = {}, names = new Set(); let cells = 0;
  const entries = sheets.map((sheet, index) => {
    const base = String(sheet.name).replace(/[\\/?*\[\]:\x00-\x1f]/g, " ").replace(/^'+|'+$/g, "").trim().slice(0, 31) || "표";
    let name = base;
    for (let n = 2; names.has(name.toLowerCase()); n++) name = base.slice(0, 31 - String(n).length - 1) + " " + n;
    names.add(name.toLowerCase());
    const width = sheet.rows.reduce((max, row) => Math.max(max, row.length), 0);
    if (!width || width > 16384 || sheet.rows.length > 1048576) throw new Error("엑셀의 행·열 한도를 초과했습니다.");
    const rows = sheet.rows.map((row, r) => `<row r="${r + 1}">${row.map((value, c) => {
      if (++cells > 500000 || String(value ?? "").length > 32767) throw new Error("자료가 너무 큽니다. 범위를 줄여 다시 내보내세요.");
      return `<c r="${column(c)}${r + 1}" t="inlineStr" s="${r === 0 ? 1 : 0}"><is><t xml:space="preserve">${xml(value)}</t></is></c>`;
    }).join("")}</row>`).join("");
    files[`xl/worksheets/sheet${index + 1}.xml`] = `<worksheet xmlns="${ns}"><sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><cols><col min="1" max="${width}" width="28" customWidth="1"/></cols><sheetData>${rows}</sheetData><autoFilter ref="A1:${column(width - 1)}${sheet.rows.length}"/></worksheet>`;
    return `<sheet name="${xml(name)}" sheetId="${index + 1}" r:id="rId${index + 1}"/>`;
  });
  files["xl/workbook.xml"] = `<workbook xmlns="${ns}" xmlns:r="${rel}"><sheets>${entries.join("")}</sheets></workbook>`;
  files["xl/styles.xml"] = `<styleSheet xmlns="${ns}"><fonts count="2"><font><sz val="11"/><name val="맑은 고딕"/></font><font><b/><sz val="11"/><name val="맑은 고딕"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFEEF1F4"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="49" fontId="1" fillId="2" borderId="0" xfId="0" applyNumberFormat="1"><alignment vertical="top" wrapText="1"/></xf></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>`;
  const relationships = body => `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${body}</Relationships>`;
  files["_rels/.rels"] = relationships(`<Relationship Id="rId1" Type="${rel}/officeDocument" Target="xl/workbook.xml"/>`);
  files["xl/_rels/workbook.xml.rels"] = relationships(sheets.map((_, i) => `<Relationship Id="rId${i + 1}" Type="${rel}/worksheet" Target="worksheets/sheet${i + 1}.xml"/>`).join("") + `<Relationship Id="styles" Type="${rel}/styles" Target="styles.xml"/>`);
  files["[Content_Types].xml"] = `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>${sheets.map((_, i) => `<Override PartName="/xl/worksheets/sheet${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>`).join("")}</Types>`;
  // ponytail: ZIP의 무압축 항목만 쓴다. 32 MiB를 넘는 업무 화면이 필요하면 스트리밍 ZIP으로 교체한다.
  const encoder = new TextEncoder(), parts = [], directory = []; let offset = 0;
  const crcTable = Array.from({ length: 256 }, (_, n) => { for (let k = 0; k < 8; k++) n = n & 1 ? 0xedb88320 ^ (n >>> 1) : n >>> 1; return n >>> 0; });
  for (const [path, content] of Object.entries(files)) {
    const name = encoder.encode(path), data = encoder.encode(content);
    if (offset + data.length > 32 * 1024 * 1024) throw new Error("파일이 32 MB를 넘습니다. 범위를 줄여 다시 내보내세요.");
    let crc = 0xffffffff; for (const byte of data) crc = crcTable[(crc ^ byte) & 255] ^ (crc >>> 8); crc = (crc ^ 0xffffffff) >>> 0;
    const local = new Uint8Array(30), lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true); lv.setUint16(4, 20, true); lv.setUint16(12, 33, true);
    lv.setUint32(14, crc, true); lv.setUint32(18, data.length, true); lv.setUint32(22, data.length, true); lv.setUint16(26, name.length, true);
    const central = new Uint8Array(46), cv = new DataView(central.buffer);
    cv.setUint32(0, 0x02014b50, true); cv.setUint16(4, 20, true); cv.setUint16(6, 20, true); cv.setUint16(14, 33, true);
    cv.setUint32(16, crc, true); cv.setUint32(20, data.length, true); cv.setUint32(24, data.length, true); cv.setUint16(28, name.length, true); cv.setUint32(42, offset, true);
    parts.push(local, name, data); directory.push(central, name); offset += local.length + name.length + data.length;
  }
  const size = directory.reduce((sum, p) => sum + p.length, 0), end = new Uint8Array(22), ev = new DataView(end.buffer);
  ev.setUint32(0, 0x06054b50, true); ev.setUint16(8, directory.length / 2, true); ev.setUint16(10, directory.length / 2, true);
  ev.setUint32(12, size, true); ev.setUint32(16, offset, true);
  const result = new Uint8Array(offset + size + end.length); let at = 0;
  for (const p of [...parts, ...directory, end]) { result.set(p, at); at += p.length; }
  return result;
}
