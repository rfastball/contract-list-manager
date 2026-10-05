import { call } from "./bridge";
import { Modal } from "./Modal";
import { useState } from "react";
import { ColumnsEditor } from "./ColumnsEditor";
import type {
  About, DataLocation, EntityType, PlanImportResult, PlanPick, Settings, UserColumn,
  WindowPrefs, WorkfileKind, WorkfilePlan,
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
  /** 설정을 닫고 나라장터 화면으로 간다. 브라우저 수집은 거기서 본다. */
  onOpenNara: () => void;
};

/**
 * 설정. 자료가 쌓이는 자리·제출과 취합·계획 엑셀·사용자 입력 열·정보(판과 라이선스).
 *
 * <p>자료는 확장과 ERP JSON 으로 들어온다(ADR-028). 그 둘을 까는 자리는 설정이 아니라 왼쪽 메뉴의 나라장터다 —
 * 맨 밑에는 그리로 가는 길만 남긴다.</p>
 */
export function SettingsPanel({
  settings, columns, onSave, onClose, onOpenNara,
  onAddColumn, onUpdateColumn, onRemoveColumn, onMoveColumn,
  onPickPlanExcel, onImportPlan, onSubmit, onMerge,
  location, onRevealData, readOnly = false, onOpenOther,
  windowPrefs = null, onSaveWindowPrefs,
  onPickWorkfile, onSwitchWorkfile, onBackup,
}: Props) {
  /** 제출본 파일 이름에 넣을 내 이름. 저장해야 파일 이름에 들어간다. */
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

  return (
    <Modal label="설정" onClose={onClose}>
        <h2>설정</h2>

        <section className="field">
          <span className="label">{readOnly ? "열람 중인 자료" : "저장 위치"}</span>

          {location ? (
            <>
              <p className="path">{location.path}</p>
              <div className="row">
                <button className="action" onClick={onRevealData}>폴더 열기</button>
                {/* 이 창은 그대로 두고 새 창으로 연다. 작업자료를 바꾸는 일이 아니다. */}
                <button className="action" onClick={onOpenOther}>다른 자료 열어 보기…</button>
              </div>
              <p className="hint">
                {readOnly ? "읽기 전용 · 원본은 바뀌지 않습니다" : "작업자료"}
                {" · "}{Math.round(location.sizeBytes / 1024).toLocaleString()}KB
                <br />
                홈 {location.home}
              </p>
            </>
          ) : (
            <p className="hint">읽는 중…</p>
          )}

          {/*
            작업자료를 바꾸는 일은 본 창에서만 한다 — 열람 창은 따로 뜬 프로세스라 거기서 바꾸면 본 창이 옛 자료를
            쥔 채 계속 고친다. 열람 창은 길만 알려 준다.
          */}
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
            <button className="action" onClick={() => void backup()} disabled={readOnly || workBusy}>
              백업 만들기…
            </button>
          </div>
          <p className="hint">
            {readOnly
              ? "이 자료로 일하려면 본 창의 「작업자료 바꾸기」를 쓰세요."
              : "옮기거나 바꾸면 창을 다시 띄웁니다. 백업은 지금 작업자료를 그대로 둡니다."}
          </p>
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
        <section className="field">
          <span className="label">제출·취합</span>

          <label className="row">
            <span className="label sub">제출자 이름</span>
            <input
              className="text"
              value={submitter}
              disabled={readOnly}
              placeholder="예: 홍길동"
              onChange={(e) => setSubmitter(e.target.value)}
            />
          </label>
          <p className="hint">설정을 저장하면 제출본 파일 이름에 반영됩니다.</p>

          <div className="row">
            <button
              className="action"
              onClick={() => void share("만드는 중…", onSubmit)}
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
        </section>

        {/* 계획은 수집이 아니라 엑셀에서 온다 — 확장·JSON 과 나란히 선 별개의 입구다. */}
        <section className="field">
          <span className="label">계획 엑셀</span>

          <p className="path">{planPath || "아직 고르지 않았습니다"}</p>

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

          {/* 열 정의도 자료 안에 있다 — 열람 중에는 보기만 한다. */}
          <fieldset className="plain" disabled={readOnly}>
            <ColumnsEditor
              columns={columns}
              onAdd={onAddColumn}
              onUpdate={onUpdateColumn}
              onRemove={onRemoveColumn}
              onMove={onMoveColumn}
            />
          </fieldset>
        </section>

        {/*
          창의 몸가짐. 자료가 아니라 이 컴퓨터의 것이라 저장 단추를 타지 않고 누르는 그 자리에서 선다.
          열람 창은 알림 영역에 두지 않으니 이 절이 서지 않는다.
        */}
        {!readOnly && windowPrefs && onSaveWindowPrefs && (
          <WindowSection prefs={windowPrefs} onSave={onSaveWindowPrefs} />
        )}

        {/* 확장 준비·상태·JSON 검토는 나라장터 화면으로 옮겼다. 찾으러 온 사람에게 길만 남긴다 — 가는 일이라 열람 중에도 열린다. */}
        <section className="field">
          <span className="label">브라우저 수집</span>
          <p className="hint">브라우저 수집은 왼쪽 메뉴의 나라장터에서 봅니다.</p>
          <div className="row">
            <button className="action" onClick={onOpenNara}>나라장터 열기</button>
          </div>
        </section>

        <div className="row end">
          <AboutLink />
          <span className="spacer" />
          <button className="action" onClick={onClose}>닫기</button>
          <button className="action primary" onClick={() => void save()} disabled={readOnly || busy}>
            {busy ? "저장 중…" : "저장"}
          </button>
        </div>
    </Modal>
  );
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
    <section className="field">
      <span className="label">창</span>
      <label className="row">
        <input
          type="checkbox"
          checked={prefs.closeToTray}
          disabled={busy}
          onChange={(e) => void save(e.target.checked, prefs.autostart)}
        />
        <span>닫아도 끝내지 않고 알림 영역에 둡니다</span>
      </label>
      <p className="hint">끝내려면 알림 영역의 아이콘을 오른쪽 단추로 눌러 「끝내기」 를 고르세요.</p>
      <label className="row">
        <input
          type="checkbox"
          checked={prefs.autostart}
          disabled={busy || !prefs.autostartAvailable}
          onChange={(e) => void save(prefs.closeToTray, e.target.checked)}
        />
        <span>Windows 에 로그인하면 자동으로 켭니다</span>
      </label>
      <p className="hint">
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
