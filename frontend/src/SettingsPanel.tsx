import { call } from "./bridge";
import { Modal } from "./Modal";
import { useEffect, useRef, useState } from "react";
import { PLANNING_ENABLED, USER_VIEWS_ENABLED } from "./features";
import { ViewPackages } from "./views/ViewPackages";
import type {
  About, DataLocation, PlanImportResult, PlanPick, Settings,
  WindowPrefs, WorkfileKind, WorkfilePlan,
} from "./types";

type Props = {
  settings: Settings;
  display: DisplayPreferences;
  onDisplayChange: (preferences: DisplayPreferences) => void;
  /** 표마다 켠 「기본으로 숨는 열」. 바꾸는 즉시 표에 선다. */
  shownColumns: ShownColumns;
  onShownColumnsChange: (shown: ShownColumns) => void;
  onExportExcel: () => Promise<void>;
  exportBusy: boolean;
  onViewsImported: () => void;
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
  /** 지금 열고 있는 파일이 든 폴더를 연다. 열람 중이면 원본의 폴더다. */
  onRevealData: () => void;
  /**
   * 열람 창인가. 그렇다면 고치는 자리(제출·취합·계획 가져오기·열 고치기·수집 도구·저장)를 모두 잠근다 —
   * 다리가 거절하지만, 눌러 본 뒤에 거절을 받게 두지 않는다.
   */
  readOnly?: boolean;
  /** 창의 몸가짐. 아직 못 읽었거나 열람 창이면 null — 그러면 「창」 절이 서지 않는다. */
  windowPrefs?: WindowPrefs | null;
  /** 창의 몸가짐을 바꾼다. 누르는 그 자리에서 선다. */
  onSaveWindowPrefs?: (closeToTray: boolean, autostart: boolean) => Promise<void>;
  /** 다른 자료를 새 창으로 열어 본다. 작업자료 창에서도 열람 창에서도 된다. */
  onOpenOther: () => void;
  /**
   * 무엇으로 바꿀지 고르게 한다. <b>아무것도 바꾸지 않는다</b> — 고른 것과 그 뜻이 온다. 그만두었거나
   * 고를 수 없는 자리였으면 null(까닭은 바깥이 알린다).
   */
  onPickWorkfile: (kind: WorkfileKind) => Promise<WorkfilePlan | null>;
  /** 확인받은 대로 바꾼다. 바꾸면 창이 다시 뜬다. */
  onSwitchWorkfile: (plan: WorkfilePlan) => Promise<void>;
  /** 지금 시점을 백업 파일로 뜬다. 자리는 앱이 묻는다. 바꾸는 일이 아니라 확인을 받지 않는다. */
  onBackup: () => Promise<void>;
  onClose: () => void;
};

export type DisplayPreferences = { fontScale: "normal" | "large" | "larger"; theme: "system" | "light" | "dark" };
const DISPLAY_KEY = "pclm.display";
export function useDisplayPreferences() {
  const [display, setDisplay] = useState<DisplayPreferences>(() => {
    try {
      const saved = JSON.parse(localStorage.getItem(DISPLAY_KEY) ?? "null");
      return { fontScale: ["normal", "large", "larger"].includes(saved?.fontScale) ? saved.fontScale : "normal",
        theme: ["system", "light", "dark"].includes(saved?.theme) ? saved.theme : "system" };
    } catch { return { fontScale: "normal", theme: "system" }; }
  });
  useEffect(() => {
    document.documentElement.dataset.fontScale = display.fontScale;
    if (display.theme === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = display.theme;
    try { localStorage.setItem(DISPLAY_KEY, JSON.stringify(display)); } catch { /* 저장 제한 중에도 현재 창에는 적용한다. */ }
  }, [display]);
  return [display, setDisplay] as const;
}

/**
 * 표마다 사람이 켠 「기본으로 숨는 열」 — 표의 뷰 이름 → 켠 열 이름. 차수 표는 본 표의 것을 따른다.
 *
 * <p>새로 담기 시작한 필드의 열은 표에서 기본으로 숨는다(ADR-043 결정 5). 무엇이 숨는지는 다리가 시트와 함께 보내고
 * (<code>Sheet.hidden</code>), 여기는 사람이 켠 것만 든다 — 켜지 않은 열은 숨고, 지금 보이는 열은 그대로 보인다.
 * 글자 크기·테마처럼 이 컴퓨터의 것이라 자료가 아니라 브라우저 저장소에 둔다.</p>
 */
export type ShownColumns = Record<string, string[]>;
const SHOWN_KEY = "pclm.shownColumns";
export function useShownColumns() {
  const [shown, setShown] = useState<ShownColumns>(() => {
    try {
      const saved: unknown = JSON.parse(localStorage.getItem(SHOWN_KEY) ?? "null");
      if (!saved || typeof saved !== "object" || Array.isArray(saved)) return {};
      return Object.fromEntries(Object.entries(saved as Record<string, unknown>)
        .filter((entry): entry is [string, unknown[]] => Array.isArray(entry[1]))
        .map(([view, names]) => [view, names.filter((n): n is string => typeof n === "string")]));
    } catch { return {}; }
  });
  useEffect(() => {
    try { localStorage.setItem(SHOWN_KEY, JSON.stringify(shown)); } catch { /* 저장 제한 중에도 현재 창에는 적용한다. */ }
  }, [shown]);
  return [shown, setShown] as const;
}

/**
 * 설정이 늘어놓는 표 — 숨는 열이 있는 표만 선다. 차수 표(<code>v_공고차수</code>·<code>v_계약차수</code>·
 * <code>v_통합차수</code>)는 본 표와 같은 열 한 벌이라 본 표의 선택을 따른다.
 */
const SHOWN_TABLES: { view: string; name: string }[] = [
  { view: "v_통합", name: "대시보드 표" },
  { view: "v_접수", name: "접수" },
  { view: "v_공고", name: "공고" },
  { view: "v_계약", name: "계약" },
];

/**
 * 설정. 자료 관리·제출과 취합·화면 표시·보기 구성·앱 실행·정보(판과 라이선스).
 *
 * <p>자료는 확장과 ERP JSON 으로 들어온다(ADR-028). 그 둘을 까는 자리는 설정이 아니라 왼쪽 메뉴의 나라장터다 —
 * 설정에는 열 관리·나라장터로 가는 중복 진입점을 두지 않는다.</p>
 */
export function SettingsPanel({
  settings, onSave, onClose, display, onDisplayChange, shownColumns, onShownColumnsChange, onExportExcel, exportBusy, onViewsImported,
  onPickPlanExcel, onImportPlan, onSubmit, onMerge,
  location, onRevealData, readOnly = false, onOpenOther,
  windowPrefs = null, onSaveWindowPrefs,
  onPickWorkfile, onSwitchWorkfile, onBackup,
}: Props) {
  /** 제출본 파일 이름에 넣을 내 이름. 제출본은 저장된 이름을 쓰므로, 만들 때 칸의 이름이 다르면 먼저 저장한다. */
  const [submitter, setSubmitter] = useState(settings.submitterName);
  const [busy, setBusy] = useState(false);

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
    } catch {
      // 바깥이 까닭을 알린다.
    } finally {
      setBusy(false);
    }
  };

  /**
   * 제출본은 앱이 DB 에 저장된 이름으로 뜬다. 칸에 적고 「저장」 을 누르지 않았으면 그 이름이 빠진 파일이 나오므로,
   * 칸의 이름이 저장된 것과 다르면 먼저 저장한다. 저장하지 못했으면 만들지 않는다.
   */
  const submit = async () => {
    if (submitter !== settings.submitterName) {
      try {
        await onSave(submitter);
      } catch {
        return;
      }
    }
    await onSubmit();
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

  /** 작업자료 동작의 바쁨. 고르는 사이·바꾸는 사이에 다른 동작을 누르지 못하게 한다. */
  const [workBusy, setWorkBusy] = useState(false);

  /** 고른 것. 확인 창이 이것을 보이고, 확인을 받아야 바꾼다. */
  const [chosen, setChosen] = useState<WorkfilePlan | null>(null);

  const pickWorkfile = async (kind: WorkfileKind) => {
    setWorkBusy(true);
    try {
      setChosen(await onPickWorkfile(kind));
    } finally {
      setWorkBusy(false);
    }
  };

  const switchWorkfile = async (confirmed: WorkfilePlan) => {
    setWorkBusy(true);
    try {
      await onSwitchWorkfile(confirmed);
    } finally {
      setWorkBusy(false);
      setChosen(null);
    }
  };

  const backup = async () => {
    setWorkBusy(true);
    try {
      await onBackup();
    } finally {
      setWorkBusy(false);
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

  /** 앱 실행 절은 작업자료 창에서, 몸가짐을 읽은 뒤에만 선다. */
  const showWindow = !readOnly && windowPrefs !== null && onSaveWindowPrefs !== undefined;
  const parts = [
    { id: "set-place", name: "자료 관리" },
    { id: "set-share", name: "제출·취합" },
    ...(PLANNING_ENABLED ? [{ id: "set-plan", name: "계획 엑셀" }] : []),
    { id: "set-display", name: "화면 표시" },
    { id: "set-columns", name: "표의 추가 열" },
    ...(USER_VIEWS_ENABLED ? [{ id: "set-views", name: "보기 구성" }] : []),
    ...(showWindow ? [{ id: "set-window", name: "앱 실행" }] : []),
  ];
  const { body, here, go, spy, release } = use목차(parts.map((part) => part.id));

  return (
    <Modal label="설정" onClose={onClose} className="settings">
      <header className="settings-head">
        <h2>설정</h2>
      </header>

      {/* 목차는 사이드바의 선 밖 자리와 같은 말투다 — 절끼리는 이어진 것이 아니라 마디도 선도 없다. 굴리면 지금 읽는 절이 깔린다. */}
      <nav className="settings-index" aria-label="설정 목차">
        {parts.map((part) => (
          <button type="button" className="nav-link" key={part.id} aria-current={here === part.id ? "true" : undefined}
            onClick={() => go(part.id)}>
            {part.name}
          </button>
        ))}
      </nav>

      <div className="settings-body" ref={body} onScroll={spy} onWheel={release} onKeyDown={release} onPointerDown={release}>
        <section className="set" id="set-place" aria-labelledby="set-place-h">
          <h3 id="set-place-h">{parts[0].name}</h3>
          <h4>{readOnly ? "열람 중인 자료" : "저장 위치"}</h4>

          {location ? (
            <div className="file-card">
              <p className="file-path">{location.path}</p>
              <p className="file-meta">
                {readOnly ? "읽기 전용 · 원본은 바뀌지 않습니다" : "작업자료"}
                {" · "}{Math.round(location.sizeBytes / 1024).toLocaleString()}KB
                <br />
                홈 {location.home}
              </p>
              <div className="row">
                <button className="action" onClick={onRevealData}>폴더 열기</button>
                {/* 이 창은 그대로 두고 새 창으로 연다. 작업자료를 바꾸는 일이 아니다. */}
                <button className="action" onClick={onOpenOther}>다른 자료 열어 보기…</button>
              </div>
            </div>
          ) : (
            <p className="hint">읽는 중…</p>
          )}

          {/*
            작업자료를 바꾸는 일은 본 창에서만 한다 — 열람 창은 따로 뜬 프로세스라 거기서 바꾸면 본 창이 옛 자료를
            쥔 채 계속 고친다. 열람 창은 본 창에서 바꾸도록 안내한다. 백업은 별도 묶음에 둔다.
          */}
          <div className="set-group">
            <div className="row">
              <button className="action" onClick={() => void pickWorkfile("move")} disabled={readOnly || workBusy}>
                작업자료 옮기기…
              </button>
              <button className="action" onClick={() => void pickWorkfile("switch")} disabled={readOnly || workBusy}>
                작업자료 바꾸기…
              </button>
              <button className="action" onClick={() => void pickWorkfile("new")} disabled={readOnly || workBusy}>
                새 계약자료…
              </button>
            </div>
            <p className="hint">
              {readOnly
                ? "이 자료로 일하려면 본 창의 「작업자료 바꾸기」를 쓰세요."
                : "옮기거나 바꾸면 창을 다시 띄웁니다."}
            </p>
          </div>
          <div className="set-group">
            <h4>백업·복구</h4>
            <p className="hint">백업 파일(.pclm)은 자료를 보관하고 복구하는 용도입니다. 복구할 때 「작업자료 바꾸기」에서 백업 파일을 고르면 그 사본을 새 작업자료로 둘 자리를 묻습니다. 지금 작업자료는 그대로 남습니다.</p>
            <button className="action" onClick={() => void backup()} disabled={readOnly || workBusy}>백업 만들기…</button>
          </div>
          <div className="set-group">
            <h4>엑셀로 공유</h4>
            <p className="hint">전체 자료를 엑셀 파일(.xlsx)로 내보냅니다. 엑셀은 열람·공유용이며, 백업 복구에는 .pclm 파일을 사용하세요. 검색 결과나 선택한 행은 각 표의 「엑셀 내보내기」에서 저장합니다.</p>
            <button className="action" onClick={() => void onExportExcel()} disabled={exportBusy}>{exportBusy ? "내보내는 중…" : "전체 자료 내보내기"}</button>
          </div>
        </section>

        {chosen && (
          <ConfirmSwitch
            plan={chosen}
            busy={workBusy}
            onConfirm={() => void switchWorkfile(chosen)}
            onClose={() => setChosen(null)}
          />
        )}

        {/* 제출·취합은 파일을 주고받는 일이라, 그 파일이 어디에 쌓이는지 바로 다음에 선다. */}
        <section className="set" id="set-share" aria-labelledby="set-share-h">
          <h3 id="set-share-h">제출·취합</h3>

          {/* 「저장」 이 저장하는 것은 이 이름 하나다 — 나머지는 누르는 그 자리에서 선다. 그래서 이름 옆에 둔다. */}
          <div className="set-group">
            <div className="row">
              <label className="label sub" htmlFor="set-submitter">제출자 이름</label>
              <input
                id="set-submitter"
                className="text"
                value={submitter}
                disabled={readOnly}
                placeholder="예: 홍길동"
                onChange={(e) => setSubmitter(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter" && !readOnly && !busy) void save(); }}
              />
              <button type="button" className="action" onClick={() => void save()} disabled={readOnly || busy}>
                {busy ? "저장 중…" : "저장"}
              </button>
            </div>
            <p className="hint">이 이름이 제출본 파일 이름에 들어갑니다. 저장하지 않았으면 제출본을 만들 때 함께 저장합니다.</p>
          </div>

          <div className="set-group">
            <div className="row">
              <button
                className="action"
                onClick={() => void share("만드는 중…", submit)}
                disabled={readOnly || shareBusy !== ""}
              >
                {shareBusy === "만드는 중…" ? shareBusy : "제출본 만들기"}
              </button>
              <button
                className="action"
                onClick={() => void share("취합하는 중…", onMerge)}
                disabled={readOnly || shareBusy !== ""}
              >
                {shareBusy === "취합하는 중…" ? shareBusy : "제출본 취합하기…"}
              </button>
            </div>
            <p className="hint">
              선택한 폴더에 취합본과 결과 보고서(txt)를 저장합니다. 취합할 때마다 새로 생성합니다.
            </p>
          </div>
        </section>

        {/* 계획은 수집이 아니라 엑셀에서 온다 — 확장·JSON 과 나란히 선 별개의 입구다. */}
        {PLANNING_ENABLED && <section className="set" id="set-plan" aria-labelledby="set-plan-h">
          <h3 id="set-plan-h">계획 엑셀</h3>

          <div className="file-card">
            <p className={planPath ? "file-path" : "file-path empty"}>{planPath || "아직 고르지 않았습니다"}</p>
            <div className="row">
              <button className="action" onClick={() => void pickPlan()} disabled={readOnly || planBusy !== ""}>
                {planBusy === "고르는 중…" ? planBusy : "고르기…"}
              </button>
              <button
                className="action primary"
                onClick={() => void importPlan()}
                disabled={readOnly || planBusy !== "" || planPath === ""}
              >
                {planBusy === "넣는 중…" ? planBusy : "가져오기"}
              </button>
            </div>
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
        </section>}
        <section className="set" id="set-display" aria-labelledby="set-display-h">
          <h3 id="set-display-h">화면 표시</h3>
          <div className="display-options">
            <label htmlFor="set-font-scale">글자 크기</label>
            <select id="set-font-scale" className="text" value={display.fontScale} onChange={event => onDisplayChange({ ...display, fontScale: event.target.value as DisplayPreferences["fontScale"] })}>
              <option value="normal">100%</option><option value="large">125%</option><option value="larger">150%</option>
            </select>
            <label htmlFor="set-theme">화면 테마</label>
            <select id="set-theme" className="text" value={display.theme} onChange={event => onDisplayChange({ ...display, theme: event.target.value as DisplayPreferences["theme"] })}>
              <option value="system">시스템 설정 따르기</option><option value="light">밝게</option><option value="dark">어둡게</option>
            </select>
          </div>
          <p className="hint">바꾸는 즉시 적용되며 이 컴퓨터에 저장합니다.</p>
        </section>
        <ShownColumnsSection shown={shownColumns} onChange={onShownColumnsChange} />
        {USER_VIEWS_ENABLED && <section className="set" id="set-views" aria-labelledby="set-views-h">
          <h3 id="set-views-h">보기 구성</h3>
          <ViewPackages readOnly={readOnly} onImported={onViewsImported} />
        </section>}

        {/*
          창의 몸가짐. 자료가 아니라 이 컴퓨터의 것이라 저장 단추를 타지 않고 누르는 그 자리에서 선다.
          열람 창은 알림 영역에 두지 않으니 이 절이 서지 않는다.
        */}
        {showWindow && <WindowSection prefs={windowPrefs!} onSave={onSaveWindowPrefs!} />}

      </div>

      <footer className="settings-foot">
        <AboutLink />
        <span className="spacer" />
        <button className="action" onClick={onClose}>닫기</button>
      </footer>
    </Modal>
  );
}

/**
 * 설정 목차의 마디. 본문을 굴리면 머리가 위 가장자리를 지난 마지막 절이 「지금」 이다.
 *
 * <p>목차를 눌러 간 절은 사람이 손수 굴리기 전까지 붙든다 — 끝의 짧은 절은 위 가장자리까지 올라가지 못해, 굴림만
 * 따르면 짧은 절을 눌렀는데 목차가 이웃 절에 서는 일이 생긴다.</p>
 */
function use목차(ids: string[]) {
  const body = useRef<HTMLDivElement>(null);
  const [here, setHere] = useState(ids[0]);
  const held = useRef(false);

  const spy = () => {
    const box = body.current;
    if (!box || held.current) return;
    const top = box.getBoundingClientRect().top;
    let current = ids[0];
    if (box.scrollTop + box.clientHeight >= box.scrollHeight - 2) current = ids[ids.length - 1];
    else for (const id of ids) {
      const part = document.getElementById(id);
      if (part && part.getBoundingClientRect().top - top <= 56) current = id;
    }
    setHere(current);
  };

  const go = (id: string) => {
    held.current = true;
    setHere(id);
    const smooth = !window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    document.getElementById(id)?.scrollIntoView?.({ block: "start", behavior: smooth ? "smooth" : "auto" });
  };

  const release = () => { held.current = false; };

  return { body, here, go, spy, release };
}

/**
 * 작업자료를 바꾸기 전에 <b>무엇이 일어나는지 말한다</b> — 어느 파일이 작업자료가 되는지, 창이 다시 뜬다는 것,
 * 브라우저 수집이 이제 어디에 저장되는지, 옮기기면 옛 파일이 지워진다는 것.
 *
 * <p>고르기 창에서 자리를 고른 것만으로 바꾸지 않는다. 창이 다시 뜨고 수집 대상이 바뀌는 일이라, 고른 자리를
 * 눈으로 본 뒤에 누르게 한다.</p>
 */
function ConfirmSwitch({ plan, busy, onConfirm, onClose }: {
  plan: WorkfilePlan;
  busy: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const { title, lead, act } = {
    move: {
      title: "작업자료 옮기기",
      lead: "지금 자료를 그대로 이 자리로 옮겨 작업자료로 씁니다.",
      act: "옮기고 다시 띄우기",
    },
    use: {
      title: "작업자료 바꾸기",
      lead: "이 파일을 작업자료로 씁니다.",
      act: "바꾸고 다시 띄우기",
    },
    snapshot: {
      title: "새 계약자료로 이어 쓰기",
      lead: `${plan.sourceRoleName ?? "고른 파일"}의 사본을 이 자리에 새 계약자료로 만들어 작업자료로 씁니다.`,
      act: "만들고 다시 띄우기",
    },
    new: {
      title: "새 계약자료 만들기",
      lead: "빈 계약자료를 이 자리에 새로 만들어 작업자료로 씁니다.",
      act: "만들고 다시 띄우기",
    },
  }[plan.action];

  return (
    <Modal label={title} onClose={busy ? () => {} : onClose}>
      <h2>{title}</h2>

      <section className="field">
        <span className="label">새 작업자료</span>
        <p>{lead}</p>
        <p className="path">{plan.path}</p>
        {plan.action === "snapshot" && plan.source && (
          <p className="hint">원본 {plan.source} — 원본은 바뀌지 않습니다.</p>
        )}
      </section>

      <ul className="hint">
        <li>창을 다시 띄웁니다.</li>
        <li>
          {plan.action === "move"
            ? "브라우저 수집은 이제 새 자리에 저장합니다. 열려 있던 확장 검토 화면도 그대로 이어집니다."
            : "브라우저 수집은 이제 이 파일에 저장합니다. 열려 있던 확장 검토 화면은 다시 확인해야 합니다."}
        </li>
        {plan.action === "move" ? (
          <li><b className="warn">다시 띄울 때 지금 자리의 옛 파일을 지웁니다: {plan.current}</b></li>
        ) : (
          <li>지금 작업자료는 그대로 남습니다: {plan.current}</li>
        )}
      </ul>

      <div className="row end">
        <button className="action" onClick={onClose} disabled={busy}>취소</button>
        <button className="action primary" onClick={onConfirm} disabled={busy}>
          {busy ? "바꾸는 중…" : act}
        </button>
      </div>
    </Modal>
  );
}

/**
 * 「표의 추가 열」 절 — 표마다 기본으로 숨는 열을 늘어놓고 켜고 끈다(ADR-043 결정 5). 무엇이 숨는지는 다리의 한 벌
 * (<code>hiddenColumns</code>)을 읽는다. 누르는 그 자리에서 표에 서고 이 컴퓨터에 남는다. 열람 창에서도 된다 — 자료가 아니라
 * 보는 모양이다.
 */
function ShownColumnsSection({ shown, onChange }: { shown: ShownColumns; onChange: (shown: ShownColumns) => void }) {
  const [hidden, setHidden] = useState<Record<string, string[]> | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    call<Record<string, string[]>>("hiddenColumns").then(
      (map) => { if (live) setHidden(map); },
      (e) => { if (live) setError(e instanceof Error ? e.message : String(e)); });
    return () => { live = false; };
  }, []);

  const toggle = (view: string, column: string, on: boolean) => {
    const mine = shown[view] ?? [];
    onChange({ ...shown, [view]: on ? [...mine.filter((c) => c !== column), column] : mine.filter((c) => c !== column) });
  };

  const tables = SHOWN_TABLES.filter((t) => (hidden?.[t.view]?.length ?? 0) > 0);
  return (
    <section className="set" id="set-columns" aria-labelledby="set-columns-h">
      <h3 id="set-columns-h">표의 추가 열</h3>
      <p className="hint">새로 담기 시작한 열은 표에서 접어 둡니다. 켠 열은 그 표의 끝에 보입니다. 전체 자료 내보내기에는 늘 담깁니다.</p>
      {error ? <p className="hint" role="alert">{error}</p>
        : hidden === null ? <p className="hint">읽는 중…</p>
        : tables.map((table) => (
          <fieldset className="set-group shown-columns" key={table.view}>
            <legend>{table.name}</legend>
            {hidden[table.view].map((column) => (
              <label className="check" key={column}>
                <input
                  type="checkbox"
                  checked={(shown[table.view] ?? []).includes(column)}
                  onChange={(e) => toggle(table.view, column, e.target.checked)}
                />
                <span>{column}</span>
              </label>
            ))}
          </fieldset>
        ))}
    </section>
  );
}

/** 「창」 절. 바꾸는 사이에는 칸을 잠가 두 번 눌러 엇갈리지 않게 한다. */
function WindowSection({ prefs, onSave }: {
  prefs: WindowPrefs;
  onSave: (closeToTray: boolean, autostart: boolean) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const save = async (closeToTray: boolean, autostart: boolean) => {
    setBusy(true);
    try {
      await onSave(closeToTray, autostart);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="set" id="set-window" aria-labelledby="set-window-h">
      <h3 id="set-window-h">앱 실행</h3>
      <label className="check">
        <input
          type="checkbox"
          checked={prefs.closeToTray}
          disabled={busy}
          onChange={(e) => void save(e.target.checked, prefs.autostart)}
        />
        <span>닫아도 끝내지 않고 알림 영역에 둡니다</span>
      </label>
      <p className="hint check-hint">끝내려면 알림 영역의 아이콘을 오른쪽 단추로 눌러 「끝내기」 를 고르세요.</p>
      <label className="check">
        <input
          type="checkbox"
          checked={prefs.autostart}
          disabled={busy || !prefs.autostartAvailable}
          onChange={(e) => void save(prefs.closeToTray, e.target.checked)}
        />
        <span>Windows 에 로그인하면 자동으로 켭니다</span>
      </label>
      {prefs.autostartAvailable && prefs.autostartCopyTarget && (
        <p className="hint check-hint">
          압축 파일 안에서 켠 프로그램입니다. 자동 실행을 켜면 이 프로그램을 {prefs.autostartCopyTarget}에 복사해 두고 그
          복사본을 로그인할 때 켭니다.
        </p>
      )}
      <p className="hint check-hint">
        {prefs.autostartAvailable || !prefs.autostartReason
          ? "알림 영역 두기가 켜져 있으면 창 없이 조용히 시작합니다."
          : prefs.autostartReason}
      </p>
    </section>
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
