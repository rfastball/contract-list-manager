import { ErpTools } from "./ErpTools";
import { call } from "./bridge";
import { Modal } from "./Modal";
import { useEffect, useState } from "react";
import { ColumnsEditor } from "./ColumnsEditor";
import type {
  About, DataLocation, EntityType, ExtensionStatus, PlanImportResult, PlanPick, Settings, UserColumn,
} from "./types";

type Entity = EntityType;


type Props = {
  settings: Settings;
  /** 손으로 채우는 열. 고치면 곧바로 반영되므로 저장 단추를 타지 않는다. */
  columns: UserColumn[];
  onAddColumn: (entityType: Entity, name: string, kind: string, choices: string) => Promise<void>;
  onUpdateColumn: (
    entityType: Entity, fieldName: string, newName: string, kind: string, choices: string,
  ) => Promise<void>;
  onRemoveColumn: (entityType: Entity, fieldName: string) => Promise<void>;
  onMoveColumn: (entityType: Entity, fieldName: string, delta: number) => Promise<void>;
  /** 계획 엑셀 고르기 창을 연다. 그만두면 null. 고른 자리는 앱이 설정에 적어 둔다. */
  onPickPlanExcel: () => Promise<PlanPick | null>;
  /** 골라 둔 엑셀을 넣는다. 머리글이 어긋난 것은 던지지 않고 결과에 담겨 온다. */
  onImportPlan: () => Promise<PlanImportResult>;
  onSave: (submitterName: string) => Promise<void>;
  /** 내 자료 한 벌을 제출본 파일로 뜬다. 자리는 앱이 묻는다. */
  onSubmit: () => Promise<void>;
  /** 받은 제출본이 든 폴더를 골라 한 벌로 합친다. 결과 글도 그 폴더에 떨어진다. */
  onMerge: () => Promise<void>;
  /** 지금 열고 있는 자료의 자리. 아직 못 읽었으면 null. */
  location: DataLocation | null;
  /** 옮길 자리를 고른다. 적어 두기만 하고 옮기는 것은 다음 실행이다. 그만두면 null. */
  onMoveData: () => Promise<string | null>;
  onRevealData: () => void;
  onClose: () => void;
  onErpChanged?: () => Promise<void>;
};

/**
 * 설정. 자료가 쌓이는 자리·제출과 취합·계획 엑셀·사용자 입력 열·브라우저 수집·정보(판과 라이선스).
 *
 * <p>자료는 확장과 ERP JSON 으로 들어온다(ADR-028) — 그 둘을 까는 자리가 맨 밑의 「브라우저 수집」이다.</p>
 */
export function SettingsPanel({
  settings, columns, onSave, onClose, onErpChanged,
  onAddColumn, onUpdateColumn, onRemoveColumn, onMoveColumn,
  onPickPlanExcel, onImportPlan, onSubmit, onMerge,
  location, onMoveData, onRevealData,
}: Props) {
  /** 제출본 파일 이름에 넣을 내 이름. 저장해야 파일 이름에 들어간다. */
  const [submitter, setSubmitter] = useState(settings.submitterName);
  const [busy, setBusy] = useState(false);
  const [moving, setMoving] = useState(false);
  /** 옮기기로 적어 둔 자리. 다음 실행에 실제로 옮겨진다. */
  const [staged, setStaged] = useState<string | null>(null);

  /** 이 창에서 새로 고른 계획 엑셀. 고르지 않았으면 설정에 적힌 것이 그대로 선다. */
  const [plan, setPlan] = useState<PlanPick | null>(null);
  const [planBusy, setPlanBusy] = useState("");
  const [planResult, setPlanResult] = useState<PlanImportResult | null>(null);

  /** 제출·취합의 바쁨. 문자열 하나로 쥔다 — 둘 중 무엇을 하는 중인지가 곧 단추의 글이다. */
  const [shareBusy, setShareBusy] = useState("");

  /** 지금 가리키고 있는 계획 엑셀. 이 창에서 고른 것이 있으면 그것이, 없으면 설정의 것이 선다. */
  const planPath = plan?.path ?? settings.planPath;

  const save = async () => {
    setBusy(true);
    try {
      await onSave(submitter);
    } finally {
      setBusy(false);
    }
  };

  const pickPlan = async () => {
    setPlanBusy("고르는 중…");
    try {
      const picked = await onPickPlanExcel();
      if (!picked) return;

      setPlan(picked);
      setPlanResult(null);
    } finally {
      setPlanBusy("");
    }
  };

  const importPlan = async () => {
    setPlanBusy("넣는 중…");
    try {
      setPlanResult(await onImportPlan());
    } catch {
      // 바깥이 까닭을 알린다.
    } finally {
      setPlanBusy("");
    }
  };

  const share = async (label: string, work: () => Promise<void>) => {
    setShareBusy(label);
    try {
      await work();
    } finally {
      setShareBusy("");
    }
  };

  const move = async () => {
    setMoving(true);
    try {
      const folder = await onMoveData();
      if (folder) setStaged(folder);
    } finally {
      setMoving(false);
    }
  };

  return (
    <Modal label="설정" onClose={onClose}>
        <h2>설정</h2>

        <section className="field">
          <span className="label">저장 위치</span>

          {location ? (
            <>
              <p className="path">{location.path}</p>
              <div className="row">
                <button className="action" onClick={onRevealData}>폴더 열기</button>
                <button className="action" onClick={() => void move()} disabled={moving}>
                  {moving ? "고르는 중…" : "다른 폴더로 옮기기…"}
                </button>
              </div>
              {staged ? (
                <p className="hint">
                  <b>앱을 다시 실행하면 다음 폴더로 옮깁니다: {staged}</b><br />
                  재실행 전까지 저장한 자료도 함께 옮깁니다.
                  기존 위치의 파일은 유지합니다.
                </p>
              ) : (
                <p className="hint">
                  {location.isDefault ? "기본 위치" : "사용자 지정 위치"}
                  {" · "}
                  {Math.round(location.sizeBytes / 1024).toLocaleString()}KB
                </p>
              )}
            </>
          ) : (
            <p className="hint">읽는 중…</p>
          )}
        </section>

        {/* 제출·취합은 파일을 주고받는 일이라, 그 파일이 어디에 쌓이는지 바로 다음에 선다. */}
        <section className="field">
          <span className="label">제출·취합</span>

          <label className="row">
            <span className="label sub">제출자 이름</span>
            <input
              className="text"
              value={submitter}
              placeholder="예: 홍길동"
              onChange={(e) => setSubmitter(e.target.value)}
            />
          </label>
          <p className="hint">설정을 저장하면 제출본 파일 이름에 반영됩니다.</p>

          <div className="row">
            <button
              className="action"
              onClick={() => void share("만드는 중…", onSubmit)}
              disabled={shareBusy !== ""}
            >
              {shareBusy === "만드는 중…" ? shareBusy : "제출본 만들기"}
            </button>
            <button
              className="action"
              onClick={() => void share("취합하는 중…", onMerge)}
              disabled={shareBusy !== ""}
            >
              {shareBusy === "취합하는 중…" ? shareBusy : "제출본 취합하기…"}
            </button>
          </div>

          <p className="hint">
            선택한 폴더에 취합본과 결과 보고서(txt)를 저장합니다. 취합할 때마다 새로 생성합니다.
          </p>
        </section>

        {/* 계획은 수집이 아니라 엑셀에서 온다 — 확장·JSON 과 나란히 선 별개의 입구다. */}
        <section className="field">
          <span className="label">계획 엑셀</span>

          <p className="path">{planPath || "아직 고르지 않았습니다"}</p>

          <div className="row">
            <button className="action" onClick={() => void pickPlan()} disabled={planBusy !== ""}>
              {planBusy === "고르는 중…" ? planBusy : "고르기…"}
            </button>
            <button
              className="action primary"
              onClick={() => void importPlan()}
              disabled={planBusy !== "" || planPath === ""}
            >
              {planBusy === "넣는 중…" ? planBusy : "가져오기"}
            </button>
          </div>

          <p className="hint">
            첫 행에 열 제목이 있어야 하며, 조달요구번호와 담당자 열은 필수입니다.
            같은 조달요구번호가 있으면 기존 행을 갱신합니다.
          </p>

          {planResult && (planResult.ok ? (
            <p className="hint">
              <b>{planResult.rows}줄</b> · 신규 {planResult.created} · 갱신 {planResult.updated}
              {" · 건너뜀 "}{planResult.skipped}
              {planResult.skipped > 0 && " (조달요구번호가 빈 줄)"}
            </p>
          ) : (
            <p className="hint">
              <b className="warn">
                필수 열 누락: {planResult.missingRequired.join(", ")}. 파일에 열을 추가한 뒤 다시 가져오세요. 자료는 변경되지 않았습니다.
              </b>
            </p>
          ))}
        </section>

        <section className="field">
          <span className="label">사용자 입력 열</span>

          <ColumnsEditor
            columns={columns}
            onAdd={onAddColumn}
            onUpdate={onUpdateColumn}
            onRemove={onRemoveColumn}
            onMove={onMoveColumn}
          />
        </section>

        {/* 자주 만질 것이 아니라 맨 밑에 둔다 — 한 번 깔고 나면 들여다볼 일이 드물다. */}
        <section className="field">
          <span className="label">브라우저 수집</span>
          <ExtensionSetup />
          <ErpTools onChanged={onErpChanged} />
        </section>

        <div className="row end">
          <AboutLink />
          <span className="spacer" />
          <button className="action" onClick={onClose}>닫기</button>
          <button className="action primary" onClick={() => void save()} disabled={busy}>
            {busy ? "저장 중…" : "저장"}
          </button>
        </div>
    </Modal>
  );
}

/**
 * 판과 라이선스 전문. 일하면서 볼 것이 아니라 설정 바닥의 작은 글씨로 두고, 누르면 따로 연다.
 * 고지가 100KB 쯤이라 설정을 열 때마다 끌어오지 않고 처음 열 때 한 번만 부른다.
 */
function AboutLink() {
  const [open, setOpen] = useState(false);
  const [about, setAbout] = useState<About>();
  const [error, setError] = useState("");
  const [asked, setAsked] = useState(false);
  const show = () => {
    setOpen(true);
    if (asked) return;
    setAsked(true);
    call<About>("about").then(setAbout, (e) => setError(e instanceof Error ? e.message : String(e)));
  };
  return <>
    <button type="button" className="quiet-link" onClick={show}>라이선스 정보</button>
    {open && <Modal label="라이선스 정보" onClose={() => setOpen(false)}>
      <h2>라이선스 정보</h2>
      {about ? <>
        <p className="hint">계약 목록 {about.version} · MIT 라이선스</p>
        <pre className="notice-text" aria-label="라이선스">{about.license}</pre>
        <pre className="notice-text" aria-label="제3자 고지">{about.notices}</pre>
      </> : error ? <p role="alert">{error}</p> : <p className="hint">읽는 중…</p>}
      <div className="row end">
        <button className="action" onClick={() => setOpen(false)}>닫기</button>
      </div>
    </Modal>}
  </>;
}

/** ISO 8601 로컬 시각을 저장소 표기(YYYY/MM/DD HH:mm)로. */
const shownAt = (at: string) => at.slice(0, 16).replace("T", " ").replaceAll("-", "/");

function ExtensionSetup() {
  const [prepared, setPrepared] = useState<{ folder: string; version: string }>();
  const [status, setStatus] = useState<ExtensionStatus>();
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // 개발 실행·배포본 밖에서는 다리가 거절한다. 상태는 참고일 뿐이라 알리지 않고 지금 흐름에 맡긴다.
  const loadStatus = () => call<ExtensionStatus>("extensionStatus").then(setStatus, () => {});
  useEffect(() => { void loadStatus(); }, []);
  async function work(action: () => Promise<void>) {
    setBusy(true); setError("");
    try { await action(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  const unconnected = status?.prepared && status.contacts.length === 0;
  const steps = <ol>
    <li>사용할 브라우저의 확장 관리에서 <b>개발자 모드</b>를 켭니다.</li>
    <li><b>압축해제된 확장 프로그램을 로드합니다</b>를 누르고 {prepared ? "위 확장 폴더" : "「확장 준비」를 누르면 보이는 확장 폴더"}를 선택합니다.</li>
    <li>나라장터 탭을 새로고침하고 <b>계약 목록 — ERP 수집</b> 확장을 엽니다.</li>
  </ol>;
  return <details className="erp-tools">
    <summary>Edge · Chrome 확장 추가</summary>
    {status?.prepared && (unconnected
      ? <p className="hint">확장 파일은 준비했지만 아직 브라우저에서 연결된 적이 없습니다.</p>
      : status.contacts.map(c => {
        const name = c.browser === "기타" ? "기타 브라우저" : c.browser;
        return <p className="hint" key={c.browser}>{c.version === status.embeddedVersion
          ? `${name} 연결됨 · 확장 ${c.version} · 마지막 연결 ${shownAt(c.at)}`
          : `${name} 확장이 옛 판(${c.version})입니다. 나라장터 화면을 열면 새 판을 스스로 불러옵니다. 그대로면 확장 관리에서 새로고침하세요.`}</p>;
      }))}
    <button className="action" type="button" disabled={busy} onClick={() => void work(async () => {
      setPrepared(undefined); setCopied(false);
      const result = await call<{ folder: string; version: string }>("prepareExtension");
      setPrepared(result);
      // 되면 폴더 선택 창에 바로 붙여 넣는다. 안 되면 아래 입력칸에서 손으로 복사한다.
      try { await navigator.clipboard.writeText(result.folder); setCopied(true); } catch { /* 입력칸에 맡긴다 */ }
      void loadStatus();
    })}>{busy ? "준비 중…" : "확장 준비"}</button>
    {prepared && <>
      <p role="status" className="hint">확장 {prepared.version}을 준비했습니다. 브라우저에서 추가를 완료하세요.</p>
      {copied && <p className="hint">폴더 경로를 복사했습니다 — 폴더 선택 창 주소칸에 Ctrl+V</p>}
      <label className="field"><span className="label">확장 폴더 · 클릭 후 Ctrl+C로 복사</span>
        <input className="text" readOnly value={prepared.folder} onFocus={e => e.currentTarget.select()} />
      </label>
      <div className="row">
        {([['edge', 'Edge 확장 관리 열기'], ['chrome', 'Chrome 확장 관리 열기'], ['folder', '확장 폴더 열기']] as const).map(([target, label]) =>
          <button className="action" type="button" key={target} disabled={busy} onClick={() => void work(async () => { await call("openExtensionSetup", target); })}>{label}</button>)}
      </div>
      {steps}
      <p className="hint">앱을 새 판으로 바꾸거나 옮겨도, 앱을 한 번 켜면 확장 파일과 연결을 스스로 고치고 확장은 나라장터 화면에서 새 판을 불러옵니다.</p>
    </>}
    {!prepared && unconnected && steps}
    {error && <p role="alert">{error}</p>}
  </details>;
}
