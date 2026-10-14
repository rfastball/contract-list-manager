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
  // 표의 칸은 HTML 조각일 수 있다(<br>·단추·숨긴 풍선 도움말). 떠 있는 문서에 넣지 않고 DOMParser 로만 읽는다 —
  // 스크립트가 돌지 않고 그림도 불러오지 않는다. 줄바꿈은 셀 안의 줄로 남긴다.
  const text = value => {
    let s = String(value ?? "");
    if (/[<&]/.test(s)) {
      const body = new DOMParser().parseFromString(s.replace(/<br\s*\/?>/gi, "\n"), "text/html").body;
      body.querySelectorAll('button, script, style, img, input, select, textarea, [hidden], [style*="display:none"], [style*="display: none"]').forEach(n => n.remove());
      s = body.textContent;
    }
    return s.split(/\r?\n/).map(line => line.trim()).join("\n").trim();
  };
  // 사람이 보는 표 그대로 — 보이는 열을 화면 순서로, 화면 머리글과 표시값으로 읽는다. DataList 에는 표가 숨긴
  // 필드와 영문 원천 키가 함께 있어 그대로 내면 보던 표와 다른 것이 나간다. 단추·선택·행 번호 열은 자료가 아니다.
  const gridRows = grid => {
    const count = Number(grid.getColumnCount()) || 0, length = Number(grid.getDataLength()) || 0;
    const order = [...Array(count).keys()];
    if (typeof grid.getDisplayColumnIndex === "function") {
      const at = order.map(i => grid.getDisplayColumnIndex(i));
      if (at.every(n => typeof n === "number" && Number.isFinite(n))) order.sort((a, b) => at[a] - at[b] || a - b);
    }
    const columns = [], names = new Set();
    for (const i of order) {
      const id = clean(grid.getColumnID(i));
      if (typeof grid.getColumnVisible === "function" && String(grid.getColumnVisible(i)) === "false") continue;
      const type = typeof grid.getColumnType === "function" ? clean(grid.getColumnType(i)) : "";
      if (/^(button|checkbox|radio|image)$/i.test(type) || /^(rowNum|rowNo|rnum|chk)$/i.test(id)) continue;
      let header = "";
      try { header = text(grid.getHeaderValue(grid.getHeaderID(i))).replace(/\s+/g, " ").trim(); } catch { /* 열 ID 로 대신한다. */ }
      header ||= id;
      if (internal.test(id + " " + header)) continue;
      const values = [];
      for (let r = 0; r < length; r++) values.push(text(grid.getCellDisplayData(r, i)));
      if (!values.some(v => v !== "")) continue;
      let name = header;
      for (let n = 2; names.has(name); n++) name = header + " " + n;
      names.add(name);
      columns.push({ name, values });
    }
    return columns.length ? [columns.map(c => c.name), ...Array.from({ length }, (_, r) => columns.map(c => c.values[r]))] : null;
  };
  const viewable = grid => ["getColumnCount", "getColumnID", "getDataLength", "getCellDisplayData"].every(m => typeof grid[m] === "function");
  for (const e of document.querySelectorAll("[id]")) {
    if (!visible(e)) continue;
    const grid = component(e.id);
    if (!grid?.getDataList) continue;
    covered.push(e); // 원천 표를 못 읽어도 DOM의 일부 행을 전체 표처럼 대체하지 않는다.
    try {
      let list = grid.getDataList();
      if (typeof list === "string") list = component(list);
      if (list && typeof list === "object") {
        if (lists.has(list)) continue;
        lists.add(list);
      }
      if (viewable(grid)) {
        let rows = null;
        try { rows = gridRows(grid); } catch { /* 원천 자료로 대신한다. */ }
        if (rows) { add(label(e) || e.id, e.id, rows); continue; }
      }
      // 표 API 가 없거나 읽지 못한 표는 DataList 의 원천 필드로 대신한다.
      if (!list?.getAllJSON) throw new Error("DataList 없음");
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

// 의존성 없이 쓰는 OOXML 통합 문서. 열의 모든 값이 자릿점 숫자이거나 날짜일 때만 엑셀 숫자·날짜로 쓰고,
// 나머지는 문자열로 두어 번호의 선행 0·긴 번호·수식처럼 보이는 원문을 보존한다.
function pclmWorkbook(sheets) {
  const xml = value => String(value ?? "").replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\ufffe\uffff]/g, "")
    .replace(/_x[0-9a-f]{4}_/gi, value => "_x005F_" + value.slice(1))
    .replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;" })[c]);
  const ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
  const rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const column = n => { let s = ""; for (n++; n; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + (n - 1) % 26) + s; return s; };
  // 네 자리 이상은 자릿점이 있어야 숫자다 — 번호·사업자번호·20260921 꼴 날짜·선행 0 코드는 문자열로 남는다.
  const numeric = /^-?(?:0|[1-9]\d{0,2}(?:,\d{3})*)(?:\.(\d+))?(원)?$/;
  const dated = /^(\d{4})([-/.])(\d{2})\2(\d{2})(?: (\d{2}):(\d{2})(?::(\d{2}))?)?$/;
  // 1900-03-01 앞은 엑셀의 1900년 윤일 착오로 일련번호가 하루 어긋나 날짜로 쓰지 않는다.
  const serial = ([, y, , mo, d, h = "0", mi = "0", s = "0"]) => {
    const at = Date.UTC(+y, mo - 1, +d, +h, +mi, +s), day = new Date(at);
    const real = +y >= 1900 && day.getUTCFullYear() === +y && day.getUTCMonth() === mo - 1 && day.getUTCDate() === +d && +h < 24 && +mi < 60 && +s < 60;
    return real && at >= Date.UTC(1900, 2, 1) ? (at - Date.UTC(1899, 11, 30)) / 864e5 : null;
  };
  const formats = new Map(); // 서식 문자열 → cellXfs 번호. 0·1 은 본문·머리글 문자열이다.
  const style = code => { if (!formats.has(code)) formats.set(code, formats.size + 2); return formats.get(code); };
  // 열 하나의 모든 값이 같은 꼴일 때만 숫자·날짜로 쓴다. 하나라도 어긋나면 그 열은 화면에 적힌 문자열 그대로다.
  const typeOf = values => {
    const filled = values.filter(v => v !== "");
    if (!filled.length) return null;
    const numbers = filled.map(v => v.match(numeric));
    if (numbers.every(m => m && m[0].replace(/\D/g, "").replace(/^0+/, "").length <= 15) && new Set(numbers.map(m => !!m[2])).size === 1) {
      const decimals = Math.max(0, ...numbers.map(m => m[1]?.length ?? 0)), won = !!numbers[0][2];
      return { style: style("#,##0" + (decimals ? "." + "0".repeat(decimals) : "") + (won ? '"원"' : "")),
        value: v => String(Number(v.replace(/[,원]/g, ""))),
        shown: v => { const [whole, part = ""] = v.replace("원", "").split("."); return whole + (decimals ? "." + part.padEnd(decimals, "0") : "") + (won ? "원" : ""); } };
    }
    const dates = filled.map(v => v.match(dated));
    if (dates.every(m => m && serial(m) !== null) && new Set(dates.map(m => m[2])).size === 1) {
      // 구분자는 글자 그대로 찍는다 — 맨 '/' 는 엑셀이 지역 날짜 구분자로 바꿔 보인다.
      const time = dates.some(m => m[5]), seconds = dates.some(m => m[7]), sep = "\\" + dates[0][2];
      const clock = seconds ? " hh:mm:ss" : time ? " hh:mm" : "";
      return { style: style(`yyyy${sep}mm${sep}dd${clock}`),
        value: v => String(serial(v.match(dated))), shown: () => "0000/00/00" + clock.replace(/[hms]/g, "0") };
    }
    return null;
  };
  // 열 너비는 가장 긴 줄의 표시 폭 — 한글·한자·전각은 두 칸으로 센다.
  const wide = /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦\u{20000}-\u{3fffd}]/u;
  const span = value => Math.max(...String(value ?? "").split("\n").map(line => [...line].reduce((n, ch) => n + (wide.test(ch) ? 2 : 1), 0)));
  if (!Array.isArray(sheets) || !sheets.length || sheets.length > 200) throw new Error("내보낼 시트 수를 확인하세요. 최대 200개입니다.");
  const files = {}, names = new Set(); let cells = 0;
  const entries = sheets.map((sheet, index) => {
    const base = String(sheet.name).replace(/[\\/?*\[\]:\x00-\x1f]/g, " ").replace(/^'+|'+$/g, "").trim().slice(0, 31) || "표";
    let name = base;
    for (let n = 2; names.has(name.toLowerCase()); n++) name = base.slice(0, 31 - String(n).length - 1) + " " + n;
    names.add(name.toLowerCase());
    const width = sheet.rows.reduce((max, row) => Math.max(max, row.length), 0);
    if (!width || width > 16384 || sheet.rows.length > 1048576) throw new Error("엑셀의 행·열 한도를 초과했습니다.");
    const body = sheet.rows.slice(1);
    const kinds = Array.from({ length: width }, (_, c) => typeOf(body.map(row => String(row[c] ?? ""))));
    const cols = kinds.map((kind, c) => {
      const sample = [sheet.rows[0][c], ...body.slice(0, 500).map(row => String(row[c] ?? "")).filter(v => v !== "").map(v => kind ? kind.shown(v) : v)];
      return `<col min="${c + 1}" max="${c + 1}" width="${Math.min(60, Math.max(8, Math.max(...sample.map(span)) + 2))}" customWidth="1"/>`;
    }).join("");
    const rows = sheet.rows.map((row, r) => `<row r="${r + 1}">${row.map((value, c) => {
      const text = String(value ?? "");
      if (++cells > 500000 || text.length > 32767) throw new Error("자료가 너무 큽니다. 범위를 줄여 다시 내보내세요.");
      if (r && text === "") return ""; // 빈 칸은 쓰지 않는다 — 숫자 열에 빈 문자열 셀이 섞이지 않는다.
      if (r && kinds[c]) return `<c r="${column(c)}${r + 1}" s="${kinds[c].style}"><v>${kinds[c].value(text)}</v></c>`;
      return `<c r="${column(c)}${r + 1}" t="inlineStr" s="${r === 0 ? 1 : 0}"><is><t xml:space="preserve">${xml(value)}</t></is></c>`;
    }).join("")}</row>`).join("");
    files[`xl/worksheets/sheet${index + 1}.xml`] = `<worksheet xmlns="${ns}"><sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><cols>${cols}</cols><sheetData>${rows}</sheetData><autoFilter ref="A1:${column(width - 1)}${sheet.rows.length}"/></worksheet>`;
    return `<sheet name="${xml(name)}" sheetId="${index + 1}" r:id="rId${index + 1}"/>`;
  });
  files["xl/workbook.xml"] = `<workbook xmlns="${ns}" xmlns:r="${rel}"><sheets>${entries.join("")}</sheets></workbook>`;
  // 숫자·날짜 서식은 쓰인 것만 164번부터 단다. 스키마 순서상 numFmts 는 fonts 앞이다.
  const custom = [...formats.keys()];
  const numFmts = custom.length ? `<numFmts count="${custom.length}">${custom.map((code, i) => `<numFmt numFmtId="${164 + i}" formatCode="${xml(code)}"/>`).join("")}</numFmts>` : "";
  const typed = custom.map((_, i) => `<xf numFmtId="${164 + i}" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyAlignment="1"><alignment vertical="top"/></xf>`).join("");
  files["xl/styles.xml"] = `<styleSheet xmlns="${ns}">${numFmts}<fonts count="2"><font><sz val="11"/><name val="맑은 고딕"/></font><font><b/><sz val="11"/><name val="맑은 고딕"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFEEF1F4"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="${2 + custom.length}"><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="49" fontId="1" fillId="2" borderId="0" xfId="0" applyNumberFormat="1"><alignment vertical="top" wrapText="1"/></xf>${typed}</cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>`;
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
