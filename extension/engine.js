// 문서 만들기 엔진(WASM, crates/pclm-doc-engine) 감싸개. 사이드 패널과 시험(node)이 같은 것을 쓴다 — 바이트와 JSON만
// 주고받는다(ADR-045). 수집은 계약목록의 것이라 여기에 없다.
//
//   const engine = await loadEngine(wasmBytesOrResponse)
//   engine.vocabulary()                             열의 다른 이름과 내 정보(crates/pclm-doc-engine/vocabulary.json)
//   engine.resolve(case, context)                   건 → {values: {이름: 글}}   통합 뷰의 값 그대로 + 내 정보
//   engine.fileName(case, templateName, pattern, today)
//                                                   {name} | {missing: [칸 이름]} | {error}   pattern 이 비면 {{건명}}_{{서식이름}}
//   engine.prepare(서식)                             {report, bytes}   본문의 {{#항목}} 표시를 구조로
//   engine.make(서식, case, context, selections)     {report, doc}     report.mismatch 가 0 이 아니면 저장하지 않는다
//   engine.inspectText(글)                           글 서식(.txt)의 {fields, slots, markers, problems}
//   engine.makeText(글, case, context, selections)   {text, fields, excluded, mismatch}   복사할 글
//
// case = 호스트 document {op:"read"} 의 답({entityType, entity, values, rows, older, caseKeys} — values 는 통합 뷰의 한 줄),
// context = {me: {dept, name, phone, fax}}

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const EMPTY = new Uint8Array(0);

export async function loadEngine(source) {
  source = await source; // fetch(...) 를 그대로 넘겨도 된다.
  const { instance } = typeof Response !== "undefined" && source instanceof Response
    ? await WebAssembly.instantiateStreaming(source, {})
    : await WebAssembly.instantiate(source, {});
  const w = instance.exports;

  // 입력을 엔진 메모리에 넣고 부른 뒤 곧바로 돌려준다(부를 때마다 서식 크기만큼 새지 않게).
  function call(op, request = null, bytes = EMPTY) {
    const inputs = [encoder.encode(op), request === null ? EMPTY : encoder.encode(JSON.stringify(request)), bytes];
    const held = inputs.map((input) => {
      if (!input.length) return [0, 0];
      const ptr = w.alloc(input.length);
      new Uint8Array(w.memory.buffer, ptr, input.length).set(input);
      return [ptr, input.length];
    });
    try {
      const status = w.engine_call(...held.flat());
      const out = new Uint8Array(w.memory.buffer, w.out_ptr(), w.out_len()).slice();
      if (status !== 0) throw new Error(decoder.decode(out));
      const n = new DataView(out.buffer).getUint32(0, true);
      return { json: JSON.parse(decoder.decode(out.subarray(4, 4 + n))), bytes: out.subarray(4 + n) };
    } finally {
      for (const [ptr, len] of held) if (len) w.dealloc(ptr, len);
    }
  }

  return {
    get memoryBytes() {
      return w.memory.buffer.byteLength;
    },
    vocabulary: () => call("vocabulary").json,
    resolve: (kase, context = {}) => call("resolve", { case: kase, context }).json,
    fileName: (kase, template, pattern = "", today = "") => call("file_name", { case: kase, template, pattern, today }).json,
    prepare(template) {
      const { json, bytes } = call("prepare", null, template);
      return { report: json, bytes };
    },
    make(template, kase, context = {}, selections = {}) {
      const { json, bytes } = call("make", { case: kase, context, selections }, template);
      return { report: json, doc: bytes };
    },
    inspectText: (text) => call("inspect_text", { text }).json,
    makeText: (text, kase, context = {}, selections = {}) => call("make_text", { text, case: kase, context, selections }).json,
  };
}
