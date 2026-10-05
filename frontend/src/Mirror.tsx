import { Fragment, useEffect, useRef, useState, type FocusEvent, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { call } from "./bridge";
import { 받침, 브라우저이름, 을를, 이가, type SourceState } from "./Nara";
import type { ImportResult, MirrorField, MirrorShot, MirrorState, MirrorStatus, MirrorTab, ScreenRow, SupportedScreen } from "./types";

/**
 * 지금 보는 화면 — 셸의 셋째 기둥(ADR-036).
 *
 * <p>Chrome 에서 보고 있는 나라장터 화면이 이 작업자료에서 무엇이 되는지를 비춘다. 칸은 <b>계약면의 표기 그대로</b>다 —
 * 앱이 그 화면을 savepoint 위에 적용해 면 뷰에서 읽은 뒤 되돌린 것이라, 여기서 날짜·금액을 흉내 내지 않는다.
 * 가져오기는 확장의 저장과 같은 절차를 탄다(쓰기 잠금 · baseToken · 쪽지 확인).</p>
 *
 * <p>비출 것이 없으면(연결 안 됨 · 준비 전 · 열람 창 · 열린 탭 없음) 레일로 접혀 작업 영역에 폭을 돌려준다. 사람이 접을
 * 수도 있다 — 그렇게 접은 것만 다시 켜도 남고, 탭이 새로 들어와도 사람이 펼칠 때까지 접힌 채로 둔다.</p>
 */

const 과와 = (w: string) => w + (받침(w) ? "과" : "와");
const 은는 = (w: string) => w + (받침(w) ? "은" : "는");

type Tone = "new" | "wait" | "saved" | "off" | "error" | "reading";

/** 탭 단추의 상태 낱말. 색만으로 가르지 않는다 — 글과 아이콘을 함께 단다. */
export function 탭상태(tab: MirrorTab): { word: string; tone: Tone } {
  if (tab.state === "unsupported") return { word: "수집 안 함", tone: "off" };
  const shot = tab.shot;
  if (shot) {
    if (shot.status === "검토 대기") return { word: "검토 대기", tone: "wait" };
    if (shot.status === "저장됨") return { word: "저장됨", tone: "saved" };
    if (shot.status === "바뀐 칸") return { word: `바뀐 칸 ${shot.changes}`, tone: "new" };
    return { word: shot.status, tone: "new" };
  }
  if (tab.state === "reading") return { word: "읽는 중", tone: "reading" };
  return { word: "읽지 못함", tone: "error" };
}

/** 확장 상태 보드의 한 줄 설명. */
export function 탭설명(tab: MirrorTab): string {
  if (tab.state === "unsupported") return "수집할 수 없는 화면";
  const shot = tab.shot;
  if (shot) {
    const fixed = shot.fields.filter((f) => f.kind === "override").length;
    return {
      "새 자료": "새 자료 · 아직 가져오지 않음",
      "새 차수": "새 차수 · 아직 가져오지 않음",
      "검토 대기": `고친 칸 ${fixed}개가 화면과 다름`,
      "바뀐 칸": `바뀐 칸 ${shot.changes}개 · 아직 가져오지 않음`,
      "저장됨": `저장됨 · 차수 ${shot.seq}`,
    }[shot.status];
  }
  if (tab.state === "reading") return "읽는 중";
  return "읽지 못함";
}

/** 검토 대기인 탭. 보드의 수와 기둥 머리의 「검토 대기 N건 보기」 가 같은 것을 센다. */
export const 검토대기 = (mirror: MirrorState | null) => (mirror?.tabs ?? []).filter((t) => t.shot?.status === "검토 대기");

const 이마 = (at: string) => at.slice(11, 16);

const 아이콘 = {
  follow: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12a8 8 0 0 1 14-5.3M20 12a8 8 0 0 1-14 5.3M18 3v4h-4M6 21v-4h4" /></svg>,
  open: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m11 17-5-5 5-5M18 17l-5-5 5-5" /></svg>,
  close: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m13 17 5-5-5-5M6 17l5-5-5-5" /></svg>,
  eye: <svg viewBox="0 0 24 24" aria-hidden="true" className="dock-eye"><path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" /><circle cx="12" cy="12" r="2.8" /></svg>,
  wait: <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5M12 16h.01" /></svg>,
  saved: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7.5" /></svg>,
  new: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 6v12M6 12h12" /></svg>,
  off: <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="m5.6 5.6 12.8 12.8" /></svg>,
  chevron: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 6 6 6-6 6" /></svg>,
  wide: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" /></svg>,
  narrow: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 4v6h6M10 20v-6H4M14 10l7-7M10 14l-7 7" /></svg>,
  lock: <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>,
  arrow: <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>,
};

/** 사람이 접은 것만 남긴다. 저장소를 쓸 수 없으면(막힌 창 · 시험) 이번 실행 동안만 기억한다. */
const 접힘열쇠 = "pclm.dock.folded";
function 접힘읽기(): boolean | null {
  try { return localStorage.getItem(접힘열쇠) === "1" ? true : null; } catch { return null; }
}
function 접힘쓰기(folded: boolean | null) {
  try {
    if (folded === true) localStorage.setItem(접힘열쇠, "1");
    else localStorage.removeItem(접힘열쇠);
  } catch { /* 남기지 못해도 이번 실행에서는 접혀 있다 */ }
}

type Done = { at: string; readAt: string; cleared: number; kept: number; status: MirrorStatus };

type Props = {
  source: SourceState;
  readOnly: boolean;
  /** 탭 보고와 투영. 아직 못 읽었거나 붙어 있지 않으면 null. */
  mirror: MirrorState | null;
  /**
   * 보드의 「지금 보는 화면에서 보기」 — 바뀔 때마다 그 탭에 고정한다. <code>follow</code> 면(사이드바의 눈 단추) 따라가기를
   * 켜고 앞 탭을 비춘다.
   */
  request: { id: string; n: number; follow?: boolean } | null;
  /** 펼쳐 작업 영역을 차지하는가. 연결되어 있을 때만 펼친다 — 아니면 펼침은 쉰다. */
  wide: boolean;
  onWide: (wide: boolean) => void;
  /** 「들어온 자료에서 보기」. */
  onShowList: () => void;
  /** 가져온 뒤 — 목록·+N·흐름은 dataVersion 으로 저절로 오지만 곧바로 다시 읽는다. */
  onImported: () => void;
  onError: (message: string) => void;
};

export function Dock(props: Props) {
  const { source, readOnly, mirror, request } = props;
  const connected = !readOnly && (source.kind === "live" || source.kind === "stale");
  const 이름 = 브라우저이름((connected ? source.attached?.browser : source.lastSeen?.browser) ?? source.latest?.browser ?? "Chrome");
  const tabs = mirror?.tabs ?? [];
  // 비출 것이 없으면 저절로 접힌다. 사람의 선택(userFold)이 있으면 그것이 이긴다 — null 은 저절로를 따른다.
  const auto = !connected || !mirror || !mirror.reporting || tabs.length === 0;
  const [userFold, setUserFold] = useState<boolean | null>(접힘읽기);
  const folded = userFold ?? auto;
  // 접혀 있으면 펼칠 것도 없다 — 연결되지 않았거나 접힌 동안 펼침은 쉰다.
  const wide = props.wide && connected && !folded;
  /** 접고 펼친 뒤 초점이 갈 곳. 누른 단추가 사라지므로 옮겨 준다. */
  const refocus = useRef<"rail" | "head" | null>(null);
  const railButton = useRef<HTMLButtonElement>(null);
  const [follow, setFollow] = useState(true);
  const [chosen, setChosen] = useState<string | null>(null);
  const [pointerIn, setPointerIn] = useState(false);
  const [focusIn, setFocusIn] = useState(false);
  /** 탭·번호마다 「수집 원값으로 복원」 을 고른 칸(고를 자리). 고르지 않은 덮개는 그대로 둔다. */
  const [restore, setRestore] = useState<Record<string, string[]>>({});
  const [importing, setImporting] = useState<string | null>(null);
  const [done, setDone] = useState<Record<string, Done>>({});
  /** 결과를 받기 전의 가져오기 — 다시 누르면 같은 수집 ID 로 보낸다. */
  const pending = useRef<Record<string, string>>({});
  const [note, setNote] = useState("");
  const noteTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [announce, setAnnounce] = useState("");
  const [sameOpen, setSameOpen] = useState(false);

  const front = mirror?.front ?? null;
  const shown = tabs.find((t) => t.id === chosen) ?? tabs.find((t) => t.id === front) ?? tabs[0] ?? null;
  const shot = shown?.shot ?? null;
  const key = shown && shot ? `${shown.id}|${shot.number}` : "";
  const picks = restore[key] ?? [];
  const finished = key ? done[key] : undefined;
  const isDone = !!finished && !!shown && (shot?.status === "저장됨" || shown.readAt === finished.readAt);
  const reading = shown?.state === "reading";

  // 따라가기를 잠시 멈추는 까닭 — 보던 것이 눈앞에서 바뀌지 않게 한다. 떠나면 따라잡는다.
  const pause = importing ? "가져오는 동안은 탭을 옮겨도 그대로 둡니다"
    : picks.length > 0 && !isDone ? "고른 칸이 있어 탭을 옮겨도 그대로 둡니다"
    : pointerIn || focusIn ? "살펴보는 동안은 탭을 옮겨도 그대로 둡니다"
    : "";

  useEffect(() => {
    if (follow && !pause && front && chosen !== front) setChosen(front);
  }, [follow, pause, front, chosen]);

  useEffect(() => {
    if (!request) return;
    // 보여 달라고 불렀다 — 사람이 접어 두었어도 편다. 접힌 채로 두면 누른 단추가 아무 일도 하지 않은 것으로 보인다.
    setUserFold((u) => (u === true ? false : u));
    if (request.follow) { setFollow(true); setChosen(request.id); }
    else { setFollow(false); setChosen(request.id); }
  }, [request]);

  useEffect(() => () => clearTimeout(noteTimer.current), []);

  // 비출 것이 사라지면 사람이 펼쳐 둔 것은 거둔다 — 다시 접힌다. 사람이 접은 것은 탭이 들어와도 그대로 둔다.
  useEffect(() => {
    if (auto) setUserFold((u) => (u === false ? null : u));
  }, [auto]);
  useEffect(() => 접힘쓰기(userFold), [userFold]);
  // 접힌 기둥 뒤에 작업 영역이 숨지 않게 — 저절로 접혀도 펼침을 거둔다.
  const onWide = props.onWide;
  useEffect(() => {
    if (folded && props.wide) onWide(false);
  }, [folded, props.wide, onWide]);
  useEffect(() => {
    if (refocus.current === "rail") railButton.current?.focus();
    else if (refocus.current === "head") document.getElementById("dock-title")?.focus();
    refocus.current = null;
  }, [folded]);

  // 알림은 한 곳에서: 다 읽은 뒤 한 번, 가져온 뒤 한 번.
  const announceKey = shown ? `${shown.id}|${shown.readAt}|${shown.state}|${shot?.status ?? ""}` : "";
  useEffect(() => {
    if (!shown || shown.state === "reading") return;
    if (shot) setAnnounce(`${shot.kind} ${shot.number} · ${shot.status}`);
    else if (shown.state === "unsupported") setAnnounce(`${shown.screen} · 수집하지 않는 화면입니다`);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [announceKey]);

  const flash = (text: string) => {
    setNote(text);
    clearTimeout(noteTimer.current);
    noteTimer.current = setTimeout(() => setNote(""), 4000);
  };

  const command = (tab: MirrorTab, what: "focusTab" | "read" | "export") =>
    void call("mirrorCommand", String(tab.pid), String(tab.tabId), what).then(
      () => {
        if (what === "focusTab") flash(`${이름}의 그 탭을 앞으로 가져왔습니다`);
        if (what === "export") flash(`${이름}의 그 탭에서 엑셀 파일을 내려받습니다`);
      },
      (e: Error) => props.onError(e.message));

  const pick = (tab: MirrorTab) => {
    if (tab.id !== front) setFollow(false);
    setChosen(tab.id);
  };

  const toggleFollow = () => {
    const next = !follow;
    setFollow(next);
    if (next && front) setChosen(front);
  };

  const choose = (choiceId: string, restoring: boolean) =>
    setRestore((r) => {
      const now = (r[key] ?? []).filter((c) => c !== choiceId);
      return { ...r, [key]: restoring ? [...now, choiceId] : now };
    });

  const importNow = async () => {
    if (!shown || !shot || importing || reading || shot.blocked) return;
    const k = key;
    const tab = shown;
    const overrides = shot.fields.filter((f) => f.kind === "override").length;
    const restoring = picks;
    setImporting(tab.id);
    const captureId = pending.current[k] ?? crypto.randomUUID();
    pending.current[k] = captureId;
    try {
      const r = await call<ImportResult>("importShot", String(tab.pid), String(tab.tabId), shot.baseToken, JSON.stringify(restoring), captureId);
      delete pending.current[k];
      if (r.status === "stored") {
        setDone((d) => ({ ...d, [k]: { at: r.at, readAt: tab.readAt, cleared: restoring.length, kept: overrides - restoring.length, status: shot.status } }));
        setRestore((p) => { const next = { ...p }; delete next[k]; return next; });
        setAnnounce(`${shot.kind} ${shot.number} · 저장했습니다 · ${r.at}`);
        props.onImported();
      } else flash(r.message);
    } catch (e) {
      props.onError((e as Error).message);
    } finally {
      setImporting(null);
    }
  };

  /**
   * 접는다. 비출 것이 없어 저절로 접히는 자리면 저절로로 돌린다 — 그때 남기면 연결된 뒤에도 접힌 채로 남는다.
   * 접힌 동안 마우스·초점이 기둥을 떠났다는 소식은 오지 않으므로 여기서 거둔다.
   */
  const fold = () => {
    refocus.current = "rail";
    setUserFold(auto ? null : true);
    setPointerIn(false);
    setFocusIn(false);
    props.onWide(false);
  };

  if (folded) {
    const railState = readOnly ? "열람 창" : source.kind === "offline" ? "연결 안 됨" : !connected ? "확장 준비 전"
      : !mirror ? "탭 읽는 중" : !mirror.reporting ? "탭 받기 전" : tabs.length === 0 ? "열린 탭 없음"
      : `탭 ${tabs.length}개`;
    return (
      <section className="dock rail" aria-labelledby="dock-title">
        <button ref={railButton} type="button" className="dock-rail" aria-expanded="false" aria-label={`지금 보는 화면 — ${railState}, 펼치기`}
          onClick={() => { refocus.current = "head"; setUserFold(false); }}>
          <span className="rail-top">{아이콘.open}<span className={`live-dot${connected ? (reading ? " reading" : " on") : ""}`} aria-hidden="true" /></span>
          <span id="dock-title" className="rail-title">지금 보는 화면</span>
          <span className="rail-state">{railState}</span>
        </button>
      </section>
    );
  }

  // 한 줄로 말한다 — 무엇을 보여 주고 있고 왜 그런지. 앞 탭이 나라장터가 아니면 앞 탭(front)이 비어 마지막 탭에 머문다.
  const liveText = note ? note
    : reading ? "화면을 읽는 중"
    : follow && pause ? pause
    : follow && !front ? `${이름} 앞 탭이 나라장터가 아니라 마지막 탭을 보여 줍니다`
    : follow ? `${이름}에서 보는 탭을 따라갑니다`
    : shown?.id === front ? "이 탭에 고정 · 따라가기 꺼짐"
    : `이 탭에 고정 · ${이름}에서는 다른 탭을 보는 중`;

  const waiting = 검토대기(mirror);
  const waitAway = connected && waiting.length > 0 && !waiting.some((t) => t.id === shown?.id);

  const onBlur = (e: FocusEvent<HTMLElement>) => {
    if (e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget)) return;
    setFocusIn(false);
  };

  return (
    <section className={`dock${wide ? " wide" : ""}`} aria-labelledby="dock-title"
      onMouseEnter={() => setPointerIn(true)} onMouseLeave={() => setPointerIn(false)}
      onFocus={() => setFocusIn(true)} onBlur={onBlur}>
      <div className="dock-in">
        <div className="dock-head">
          <div className="dock-band">
            <div className="dock-title-row">
              <span className={`live-dot${connected ? (reading ? " reading" : " on") : ""}`} aria-hidden="true" />
              <h2 id="dock-title" tabIndex={-1}>지금 보는 화면</h2>
              {connected && <>
                <button type="button" className={`dock-icon${follow ? " on" : ""}`} aria-pressed={follow} aria-label={`${이름} 따라가기`}
                  title={`${이름} 따라가기 — ${이름}에서 탭을 옮기면 여기도 따라 옮깁니다`} onClick={toggleFollow}>{아이콘.follow}</button>
                <button type="button" className="dock-icon" aria-label={wide ? "좁히기" : "펼치기"}
                  title={wide ? "지금 보는 화면 좁히기" : "지금 보는 화면 펼치기"} onClick={() => props.onWide(!wide)}>
                  {wide ? 아이콘.narrow : 아이콘.wide}
                </button>
              </>}
              <button type="button" className="dock-icon" aria-expanded="true" aria-label="접기" title="지금 보는 화면 접기" onClick={fold}>{아이콘.close}</button>
            </div>
            {connected && <p className="dock-live">{liveText}</p>}
          </div>
          {connected && tabs.length > 0 && (
            <div className="dock-tabs">
              <div className="dock-tabs-head">
                <span id="dock-tabs-label">{이름}의 나라장터 탭 {tabs.length}개</span>
                {waitAway && (
                  <button type="button" className="dock-wait-go" onClick={() => { setFollow(false); setChosen(waiting[0].id); }}>
                    {아이콘.wait}검토 대기 {waiting.length}건 보기
                  </button>
                )}
              </div>
              <div role="group" aria-labelledby="dock-tabs-label" className="dock-tab-list">
                {tabs.map((t) => {
                  const on = t.id === shown?.id;
                  const { word, tone } = 탭상태(t);
                  const no = t.shot?.number ?? "";
                  return (
                    <button key={t.id} type="button" className={`dock-tab${on ? " on" : ""}`} aria-pressed={on}
                      aria-label={`${t.screen || "나라장터"}${no ? " " + no : ""}, ${word}${t.front ? `, ${이름}에서 앞에 있는 탭` : ""}${on ? ", 지금 보고 있는 탭" : ""}`}
                      onClick={() => pick(t)}>
                      <span className="dock-tab-name"><span className="dock-tab-screen">{t.screen || "나라장터"}</span><span className="dock-tab-no">{no}</span></span>
                      {t.front && 아이콘.eye}
                      <span className={`dock-badge ${tone}`}>
                        {tone === "wait" ? 아이콘.wait : tone === "saved" ? 아이콘.saved : tone === "off" ? 아이콘.off : tone === "new" ? 아이콘.new : null}
                        {word}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        <p className="sr-only" role="status">{connected ? announce : ""}</p>

        <div className={`dock-body${wide ? " wide" : ""}`}>
          {!connected ? <Unknown readOnly={readOnly} kind={source.kind} name={이름} />
            : !mirror ? <Quiet title={`${이름}의 나라장터 탭을 읽는 중입니다.`} />
            : !mirror.reporting ? <Quiet title="확장이 아직 열린 탭을 보내지 않았습니다."
                sub={source.kind === "stale" ? "확장이 옛 판이라 열린 탭을 보내지 않습니다. 새 판으로 바뀌면 여기에 나타납니다." : "잠시 뒤 자동으로 나타납니다."} />
            : !shown ? <Quiet title={`${이름}에 열린 나라장터 탭이 없습니다.`} sub="나라장터에서 접수·공고·계약 상세 화면을 열면 여기에 나타납니다." />
            : wide && (shown.state === "unsupported" || shot) ? (
              <Wide key={`${shown.id}|${shown.readAt}`} name={이름} tab={shown} shot={shot} reading={reading} done={isDone ? finished! : undefined}
                picks={picks} onChoose={choose} screens={mirror.screens ?? []} onExport={() => command(shown, "export")}
                footer={shot && (
                  <Footer wide name={이름} shot={shot} done={isDone ? finished! : undefined} importing={importing === shown.id} reading={reading}
                    busy={importing !== null} picks={picks}
                    onGo={() => command(shown, "focusTab")} onImport={() => void importNow()} onShowList={props.onShowList} />
                )} />
            )
            : shown.state === "unsupported" ? <Unsupported tab={shown} screens={mirror.screens ?? []} onExport={() => command(shown, "export")} />
            : shot ? <Projection key={`${shown.id}|${shown.readAt}`} tab={shown} shot={shot} reading={reading} done={isDone ? finished! : undefined}
                picks={picks} onChoose={choose} sameOpen={sameOpen} onToggleSame={() => setSameOpen((v) => !v)} />
            : shown.state === "reading" ? <Quiet title="화면을 읽는 중입니다." />
            : (
              <section className="dock-note" aria-label="읽지 못한 화면">
                <h3>이 화면을 읽지 못했습니다</h3>
                <p className="soft">{shown.error || shown.message || "화면을 다시 읽어 보세요."}</p>
                <button type="button" className="action" onClick={() => command(shown, "read")}>다시 읽기</button>
              </section>
            )}
        </div>

        {connected && !wide && shown && shot && (
          <Footer name={이름} shot={shot} done={isDone ? finished! : undefined} importing={importing === shown.id} reading={reading}
            busy={importing !== null} picks={picks}
            onGo={() => command(shown, "focusTab")} onImport={() => void importNow()} onShowList={props.onShowList} />
        )}
      </div>
    </section>
  );
}

function Quiet({ title, sub }: { title: string; sub?: string }) {
  return (
    <div className="dock-note">
      <p className="strong">{title}</p>
      {sub && <p className="soft">{sub}</p>}
    </div>
  );
}

function Unknown({ readOnly, kind, name }: { readOnly: boolean; kind: SourceState["kind"]; name: string }) {
  const [title, sub] = readOnly
    ? ["열람 창에서는 지금 보는 화면을 쓸 수 없습니다.", "작업자료 창에서 보고 가져올 수 있습니다."]
    : kind === "offline"
      ? [`${이가(name)} 연결되어 있지 않아 지금 보는 화면을 알 수 없습니다.`,
        `${을를(name)} 켜면 자동으로 다시 연결되고, 앞에 있는 나라장터 탭이 여기에 나타납니다.`]
      : [`확장을 준비하면 ${name}에서 보고 있는 나라장터 화면이 여기에 나타납니다.`, "확장 준비는 「들어온 자료」에서 합니다."];
  return <Quiet title={title} sub={sub} />;
}

/**
 * 수집하지 않는 화면(ADR-037). 무엇을 열면 되는지 말하고, 내보내기는 확장의 그 기능을 부른다. 목록은 앱이 매핑에서 읽어 준
 * 것 그대로다 — 여기에 따로 적으면 받는 화면을 넓혀도 안내만 옛것으로 남는다.
 */
function Unsupported({ tab, screens, onExport }: { tab: MirrorTab; screens: SupportedScreen[]; onExport: () => void }) {
  return (
    <section className="dock-note" aria-label="수집할 수 없는 화면">
      <h3>이 화면은 수집하지 않습니다</h3>
      {tab.menu && <p className="dock-menu">화면 번호 {tab.menu}</p>}
      {screens.length > 0 && <>
        <p className="soft">수집하는 화면은 아래와 같습니다.</p>
        <dl className="dock-rules">
          {screens.map((s) => <Fragment key={s.code}><dt>{s.kind}</dt><dd>{s.name}</dd></Fragment>)}
        </dl>
      </>}
      <button type="button" className="action" onClick={onExport}>현재 화면 엑셀로 내보내기</button>
    </section>
  );
}

/** 견준 차수 — 「차수 00과」. 견줄 차수가 없으면(새 자료) 빈 글. */
const 견줌 = (shot: MirrorShot) => (shot.compareSeq === null ? "" : 과와(`차수 ${shot.compareSeq}`));

/** 정체의 알약과 한 줄 — 좁은 기둥과 펼친 보기가 같은 말을 한다. */
function 판정(shot: MirrorShot, done: Done | undefined) {
  const cmpHead = 견줌(shot);
  const changed = shot.fields.filter((f) => f.kind === "changed").length;
  const overrides = shot.fields.filter((f) => f.kind === "override").length;
  const verdict = done ? "저장됨" : shot.status === "바뀐 칸" ? `바뀐 칸 ${shot.changes}` : shot.status;
  const tone = done || shot.status === "저장됨" ? "ok" : shot.status === "검토 대기" ? "warn" : "new";
  const sub = done
    ? done.status === "새 차수" || done.status === "새 자료"
      ? `${shot.kind} ${shot.base}에 ${을를(`차수 ${shot.seq}`)} 추가했습니다.`
      : done.cleared + done.kept > 0
        ? `${done.cleared ? `고친 값 ${done.cleared}개를 지우고 ` : ""}${done.kept
          ? `${done.cleared ? `나머지 ${done.kept}개는` : "고친 값은"} 그대로 두고 저장했습니다. 수집 원값은 따로 남아 있습니다.`
          : "저장했습니다."}`
        : "저장했습니다."
    : {
      "새 자료": `작업자료에 아직 없는 ${shot.kind}입니다. 가져오면 새로 추가됩니다.`,
      "새 차수": `작업자료에는 차수 ${shot.latestSeq}까지 있습니다. ${cmpHead} 다른 칸 ${changed}개, 품목 ${shot.itemChanges}행.`,
      "검토 대기": `작업자료의 차수 ${shot.seq}에서 사람이 고친 칸 ${overrides}개가 화면의 값과 다릅니다.`,
      "바뀐 칸": `작업자료의 ${cmpHead} 다른 칸 ${changed}개, 품목 ${shot.itemChanges}행.`,
      "저장됨": `작업자료의 ${cmpHead} 같은 화면입니다.`,
    }[shot.status];
  return { verdict, tone, sub };
}

/** 자리 한 칸의 말. */
const 자리말 = (p: MirrorShot["place"][number]) =>
  p.state === "here" ? "이 화면"
  : p.state === "linked" ? "연결됨"
  : p.state === "ref" ? `참조 ${p.number} · ${p.collected ? "연결되지 않음" : "아직 수집되지 않음"}`
  : "아직 수집되지 않음";

/** 차수 한 줄의 말. */
const 차수말 = (r: MirrorShot["rounds"][number], done: Done | undefined) =>
  r.state === "ghost" ? (done ? `방금 저장 · ${done.at}` : "가져오면 여기에 추가됩니다")
  : r.state === "current" ? (done ? "이 화면 · 방금 다시 저장" : `이 화면${r.savedOn ? ` · ${r.savedOn} 저장` : ""}`)
  : r.savedOn ? `${r.savedOn} 저장` : "";

/** 품목 한 줄이 견준 차수와 어떻게 다른가. 같으면 빈 글. */
const 품목다름 = (it: MirrorShot["items"][number], cmp: string | null) =>
  it.before ? `${은는(`차수 ${cmp}`)} ${it.before.quantity}${it.before.unit} · ${it.before.amount || it.before.price}`
  : it.added ? `차수 ${cmp}에 없던 행` : "";

/** 이 화면이 무엇이 되는가 — 정체, 자리, 칸, 품목. */
function Projection({ tab, shot, reading, done, picks, onChoose, sameOpen, onToggleSame }: {
  tab: MirrorTab; shot: MirrorShot; reading: boolean; done: Done | undefined; picks: string[];
  onChoose: (choiceId: string, restoring: boolean) => void; sameOpen: boolean; onToggleSame: () => void;
}) {
  const cmp = shot.compareSeq;
  const cmpHead = 견줌(shot);
  const changed = shot.fields.filter((f) => f.kind === "changed");
  const overrides = shot.fields.filter((f) => f.kind === "override");
  const fresh = shot.fields.filter((f) => f.kind === "new");
  const same = shot.fields.filter((f) => f.kind === "same");
  const ids = shot.fields.filter((f) => f.kind === "id");
  const key = [...changed, ...overrides, ...fresh];
  const { verdict, tone, sub } = 판정(shot, done);

  const keyHead = overrides.length > 0 ? `사람이 고친 칸 ${overrides.length}개 · 화면과 다름`
    : fresh.length > 0 ? `새로 저장될 칸 ${fresh.length}개`
    : changed.length > 0 ? `${cmpHead} 다른 칸 ${changed.length}개`
    : `${cmpHead} 다른 칸 없음`;

  const itemsNote = shot.itemsAllRead ? `화면의 품목 표 ${shot.itemRows}행을 모두 읽었습니다`
    : shot.itemRows === 0 ? "화면에서 품목 표를 읽지 않았습니다" : "";

  return (
    <>
      <section aria-label="이 화면의 자료" className={`dock-id${reading ? " reading" : ""}`}>
        {reading && <span className="dock-scan" aria-hidden="true" />}
        <div className="dock-pills"><span className="kind-pill">{shot.kind}</span><span className={`pill ${tone}`}>{verdict}</span></div>
        <p className="dock-no">{shot.number}</p>
        <p className="dock-name">{shot.title}</p>
        {sub && <p className="soft small">{sub}</p>}
        <p className="soft tiny num">{reading ? "화면을 읽는 중" : tab.readAt ? `${이마(tab.readAt)}에 읽음` : ""}</p>
      </section>

      <Place shot={shot} done={done} />

      <section aria-label="저장될 칸" className="dock-fields">
        <div className="dock-sec-head"><h3>{keyHead}</h3><span className="soft tiny">{shot.view} · {shot.fields.length}열</span></div>
        {ids.length > 0 && (
          <p className="dock-idline num">
            {ids[0].column} {ids[0].value}{ids.length > 1 && ` → ${ids.slice(1).map((f) => `${f.column} ${f.value}`).join(" · ")}`}
          </p>
        )}
        {key.map((f, i) => <FieldRow key={f.column} field={f} index={i} picks={picks} onChoose={onChoose} />)}
        {same.length > 0 && (
          <>
            <button type="button" className="dock-same" aria-expanded={sameOpen} onClick={onToggleSame}>
              {아이콘.chevron}같은 칸 {same.length}개
            </button>
            {sameOpen && (
              <dl className="dock-same-list">
                {same.map((f) => <div key={f.column}><dt>{f.column}</dt><dd className="num">{f.value}</dd></div>)}
              </dl>
            )}
          </>
        )}
        {shot.rest > 0 && <p className="dock-rest soft tiny">나머지 {shot.rest}열 · 이 화면에서는 읽지 않음</p>}
      </section>

      <section aria-label="품목" className="dock-items">
        <div className="dock-sec-head"><h3>품목</h3><span className="soft tiny">{itemsNote}</span></div>
        {shot.items.length === 0 && <p className="dock-rest soft tiny">품목이 없습니다.</p>}
        {shot.items.map((it, i) => {
          const change = 품목다름(it, cmp);
          return (
            <div key={it.line} className="dock-item" data-motion style={{ animationDelay: `${(i + 4) * 36}ms` }}>
              <span className="soft num">{it.line}</span>
              <span className="dock-item-main">
                <span className="clip">{it.name} {it.spec && <span className="soft tiny">{it.spec}</span>}</span>
                <span className="soft tiny num">{it.quantity}{it.unit} × {it.price}</span>
                {change && <span className="dock-change num">{change}</span>}
              </span>
              <span className={`num dock-sum${change ? " changed" : ""}`}>{it.amount || it.price}</span>
            </div>
          );
        })}
      </section>
    </>
  );
}

function FieldRow({ field: f, index, picks, onChoose }: {
  field: MirrorField; index: number; picks: string[]; onChoose: (choiceId: string, restoring: boolean) => void;
}) {
  const restoring = f.choiceId !== null && picks.includes(f.choiceId);
  return (
    <div className="dock-field" data-motion style={{ animationDelay: `${90 + index * 36}ms` }}>
      <div className="dock-field-col">{f.column}</div>
      {f.kind === "override" ? (
        <>
          <dl className="dock-override">
            <dt>화면</dt><dd className={`num${restoring ? "" : " soft"}`}>{f.value || "빈 값"}</dd>
            <dt>고친 값</dt>
            <dd><span className="dock-human">{restoring ? <><s>{f.human}</s> <b className="danger">지움</b></> : f.human}</span></dd>
          </dl>
          <span role="group" aria-label={`${f.column} — 고친 값을 둘지, 지우고 수집 원값을 쓸지`} className="dock-seg">
            <button type="button" aria-pressed={!restoring} onClick={() => onChoose(f.choiceId!, false)}>고친 값 그대로</button>
            <button type="button" aria-pressed={restoring} onClick={() => onChoose(f.choiceId!, true)}>수집 원값으로 복원</button>
          </span>
        </>
      ) : (
        <>
          <div className="dock-field-val">
            <span className={`num${f.kind === "changed" ? " changed" : ""}`}>{f.value}</span>
            {f.kind === "changed" && <s className="soft num">{f.old || "빈 값"}</s>}
          </div>
          {f.raw && <div className="soft tiny num">화면 표기 {f.raw}</div>}
        </>
      )}
    </div>
  );
}

/** 작업자료에서의 자리 — 왼쪽 메뉴의 가지와 같은 줄기·마디. 이 화면의 차수들은 그 칸 아래에 선다. */
function Place({ shot, done }: { shot: MirrorShot; done: Done | undefined }) {
  const note = 자리말;
  return (
    <section aria-label="작업자료에서의 자리" className="dock-place">
      <ol>
        {shot.place.map((p) => (
          <li key={p.kind} className={`place-${p.state}${p.state === "here" && done ? " landed" : ""}`}>
            <span className="place-node" aria-hidden="true" />
            <span className="place-text">
              <span className="place-head"><span className="place-kind">{p.kind}</span>{(p.state === "here" || p.state === "linked") && <b className="num">{p.number}</b>}</span>
              <span className="soft tiny">{note(p)}</span>
              {p.state === "here" && (
                <ol className="dock-rounds">
                  {shot.rounds.map((r) => {
                    const ghost = r.state === "ghost" && !done;
                    const landed = r.state === "ghost" && !!done;
                    return (
                      <li key={r.seq} className={`round ${ghost ? "ghost" : landed ? "landed" : r.state}`}>
                        <span className="strong">차수 {r.seq}</span>
                        <span className="num">{r.amount}</span>
                        <span className="round-note">{차수말(r, done)}</span>
                      </li>
                    );
                  })}
                </ol>
              )}
            </span>
          </li>
        ))}
      </ol>
    </section>
  );
}

/** 가져오기 띠. 단추에 번호를 적어, 누르는 순간 무엇을 가져오는지 단추가 말하게 한다. */
function Footer({ wide, name, shot, done, importing, reading, busy, picks, onGo, onImport, onShowList }: {
  /** 펼친 보기에서는 오른쪽 기둥의 끝에 붙는다. */
  wide?: boolean;
  name: string; shot: MirrorShot; done: Done | undefined; importing: boolean; reading: boolean; busy: boolean; picks: string[];
  onGo: () => void; onImport: () => void; onShowList: () => void;
}) {
  const overrides = shot.fields.filter((f) => f.kind === "override");
  const restoring = overrides.filter((f) => f.choiceId !== null && picks.includes(f.choiceId));
  const blocked = shot.blocked !== null;
  const title: ReactNode = done ? `저장했습니다 · ${done.at}`
    : importing ? `${shot.number} 가져오는 중`
    : blocked ? `${name}의 수집기에서 검토하세요`
    : restoring.length ? `고친 값 ${restoring.length}개를 지우고 가져옵니다`
    : overrides.length ? `고친 값 ${overrides.length}개는 그대로 둡니다`
    : {
      "새 자료": `가져오면 ${이가(`${shot.kind} ${shot.number}`)} 새로 추가됩니다`,
      "새 차수": `가져오면 ${이가(`차수 ${shot.seq}`)} 새로 추가됩니다`,
      "검토 대기": "",
      "바뀐 칸": `가져오면 바뀐 칸 ${shot.changes}개를 고칩니다`,
      "저장됨": "작업자료와 같습니다",
    }[shot.status];
  const sub = done ? `「들어온 자료」와 ${shot.kind} 목록에 추가했습니다.`
    : blocked ? shot.blocked
    : restoring.length ? `${restoring.map((f) => f.column).join(" · ")}의 고친 값을 지우고 화면의 값을 씁니다. 지운 값은 되돌릴 수 없습니다.`
    : overrides.length ? "가져와도 고친 값은 바뀌지 않습니다. 화면의 값을 쓰려면 그 칸에서 「수집 원값으로 복원」을 고르세요."
    : shot.status === "새 차수" ? `${은는(`차수 ${shot.latestSeq}`)} 그대로 남습니다. ${name}에서 Alt+Shift+S를 눌러도 똑같이 가져옵니다.`
    : shot.status === "저장됨" ? "가져와도 바뀌는 것이 없습니다."
    : `${name}에서 Alt+Shift+S를 눌러도 똑같이 가져옵니다.`;
  const label = importing ? `${shot.number} 가져오는 중…`
    : restoring.length ? `고친 값 ${restoring.length}개를 지우고 ${shot.number} 가져오기`
    : `${shot.number} 가져오기`;
  const body = (
    <>
      <div>
        <div className={`dock-act-title${done ? " ok" : restoring.length && !importing ? " danger" : ""}`}>{title}</div>
        <div className="soft tiny">{sub}</div>
      </div>
      <div className="dock-act-row">
        {done
          ? <button type="button" className="action" onClick={onShowList}>들어온 자료에서 보기</button>
          : (
            <>
              <button type="button" className="action" onClick={onGo}>나라장터 탭으로 가기</button>
              <button type="button" className="action primary" disabled={blocked || reading || busy} onClick={onImport}>{label}</button>
            </>
          )}
      </div>
    </>
  );
  return wide
    ? <section aria-label="가져오기" className="dock-foot wide">{body}</section>
    : <div className="dock-foot">{body}</div>;
}

// ── 펼침: 화면과 구조를 나란히 ─────────────────────────────

/**
 * 두 자리가 닿는가 — 같거나, 표 자리(<code>table:x</code>)가 그 표의 줄(<code>table:x:n</code>)을 품는다.
 * Core 의 <code>Mirror.Points</code> 와 같은 규칙이다.
 */
export const 잇는가 = (a: string, b: string) =>
  a !== "" && b !== "" && (a === b || (a.startsWith("table:") && b.startsWith(a + ":")) || (b.startsWith("table:") && a.startsWith(b + ":")));

/** 밝힌 것 — 그 요소와, 그것이 가리키는 화면의 자리들. */
type Lit = { key: string; sources: string[] };

/** 목록 안에서 화살표·Home·End 로 초점을 옮긴다. 초점이 밝힘을 데려간다. */
const rove = (horizontal: boolean) => (e: KeyboardEvent<HTMLElement>) => {
  const k = e.key;
  const fwd = k === "ArrowDown" || (horizontal && k === "ArrowRight");
  const back = k === "ArrowUp" || (horizontal && k === "ArrowLeft");
  if (!fwd && !back && k !== "Home" && k !== "End") return;
  const t = e.target as HTMLElement;
  if (!t.dataset?.link) return;
  const all = [...e.currentTarget.querySelectorAll<HTMLElement>("[data-link]")];
  const i = all.indexOf(t);
  if (i < 0) return;
  const n = k === "Home" ? 0 : k === "End" ? all.length - 1 : fwd ? Math.min(all.length - 1, i + 1) : Math.max(0, i - 1);
  e.preventDefault();
  all[n].focus();
};

/**
 * 펼친 보기. 왼쪽은 Chrome 의 화면 그대로(확장이 읽어 보낸 이름표와 보이는 글), 오른쪽은 작업자료에 설 모양이다.
 * 같은 자리를 가리키는 왼쪽 줄·오른쪽 칸·품목 줄·자리 카드가 함께 밝아진다 — 마우스를 올리거나, 목록마다 Tab 한 번으로
 * 들어가 화살표로 옮긴다. 둘 다 떠나면 밝힘을 거둔다. 가리키는 것이 없을 때는 첫 「바뀜」 칸(없으면 첫 고친 값)을 밝혀 둔다.
 */
function Wide({ name, tab, shot, reading, done, picks, onChoose, screens, onExport, footer }: {
  name: string; tab: MirrorTab; shot: MirrorShot | null; reading: boolean; done: Done | undefined; picks: string[];
  onChoose: (choiceId: string, restoring: boolean) => void; screens: SupportedScreen[]; onExport: () => void; footer: ReactNode;
}) {
  const [lit, setLit] = useState<Lit | null>(null);
  const pointer = useRef<Lit | null>(null);
  const links = new Map<string, string[]>();

  const fallback = shot ? shot.fields.find((f) => f.kind === "changed") ?? shot.fields.find((f) => f.kind === "override") : undefined;
  const current = lit ?? (fallback ? { key: `f:${fallback.column}`, sources: fallback.sources } : null);
  const on = (key: string, sources: string[]) =>
    !!current && (current.key === key || sources.some((a) => current.sources.some((b) => 잇는가(a, b))));

  /** 잇는 요소 하나의 손잡이. 목록마다 Tab 자리는 하나 — 밝힌 것(없으면 첫 것)만 0. */
  const link = (key: string, sources: string[], stop: boolean) => {
    links.set(key, sources);
    return {
      "data-link": key,
      tabIndex: stop ? 0 : -1,
      onMouseEnter: () => { pointer.current = { key, sources }; setLit({ key, sources }); },
      onFocus: () => setLit({ key, sources }),
    };
  };
  const stopOf = (list: { key: string; sources: string[] }[]) => (list.find((x) => on(x.key, x.sources)) ?? list[0])?.key;

  const leave = (e: MouseEvent<HTMLDivElement>) => {
    pointer.current = null;
    const a = document.activeElement;
    const key = a instanceof HTMLElement && e.currentTarget.contains(a) ? a.dataset.link : undefined;
    setLit(key ? { key, sources: links.get(key) ?? [] } : null);
  };
  const blur = (e: FocusEvent<HTMLDivElement>) => {
    const to = e.relatedTarget;
    if (to instanceof HTMLElement && to.dataset.link && e.currentTarget.contains(to)) return;
    setLit(pointer.current);
  };

  // 왼쪽 — 구역마다 묶는다. 구역 제목을 찾지 못한 줄은 한 묶음이다.
  const supported = !!shot;
  const cmpHead = shot ? 견줌(shot) : "";
  const rows = tab.screenRows.map((r, i) => ({ ...r, key: `s:${i}`, linked: supported && r.source !== "" }));
  const groups: { h: string; rows: typeof rows }[] = [];
  for (const r of rows) {
    const last = groups[groups.length - 1];
    if (last && last.h === r.group) last.rows.push(r);
    else groups.push({ h: r.group, rows: [r] });
  }
  const leftStop = stopOf(rows.filter((r) => r.linked).map((r) => ({ key: r.key, sources: [r.source] })));
  const differs = (r: ScreenRow) => !!shot && (
    shot.fields.some((f) => f.kind === "changed" && f.sources.some((s) => 잇는가(s, r.source))) ||
    shot.items.some((it) => (it.before !== null || it.added) && it.source !== "" && it.source === r.source));

  const caption = supported
    ? `${name}에서 보고 있는 화면을 줄인 것입니다. 칸에 마우스를 올리거나 Tab·화살표로 옮기면 오른쪽에 그 값이 저장될 칸이 강조됩니다.` +
      (fallback ? ` 가리키는 칸이 없으면 ${을를(fallback.column)} 강조합니다.` : "") +
      (cmpHead ? ` 파란 점은 ${cmpHead} 다른 칸입니다.` : "")
    : `${name}에서 보고 있는 화면을 줄인 것입니다.`;

  return (
    <div className="wide-grid" onMouseLeave={leave} onBlur={blur}>
      <figure className="wide-screen">
        <div className="wide-frame">
          <div className="wide-url">{아이콘.lock}<span className="clip">www.g2b.go.kr › {tab.screen || "나라장터"}</span></div>
          <div className="wide-page" onKeyDown={rove(false)}>
            <div className="wide-page-title">{tab.screen || "나라장터"}</div>
            {rows.length === 0 && <p className="soft small">화면의 항목 이름을 아직 받지 못했습니다. 확장을 새 판으로 바꾼 뒤 다시 읽어 보세요.</p>}
            {groups.map((g, gi) => (
              <div className="wide-group" key={`${gi}-${g.h}`}>
                {g.h && <div className="wide-group-h">{g.h}</div>}
                <div role="table" aria-label={g.h || tab.screen || "화면"}>
                  {g.rows.map((r) => (
                    <div role="row" key={r.key} className={`wide-src${r.linked && on(r.key, [r.source]) ? " lit" : ""}`}
                      {...(r.linked ? link(r.key, [r.source], r.key === leftStop) : {})}>
                      <span role="rowheader" className="clip">{r.label || " "}</span>
                      <span role="cell" className="wide-src-val">
                        <span className="clip num">{r.text}</span>
                        {differs(r) && <span role="img" aria-label={`${cmpHead} 다름`} className="wide-dot" />}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
          {reading && <span className="dock-scan" aria-hidden="true" />}
        </div>
        <figcaption className="soft tiny">{caption}</figcaption>
      </figure>

      <div className="wide-side">
        {!shot ? <Unsupported tab={tab} screens={screens} onExport={onExport} /> : (
          <WideShot shot={shot} tab={tab} reading={reading} done={done} picks={picks} onChoose={onChoose}
            on={on} link={link} stopOf={stopOf} />
        )}
        {footer}
      </div>
    </div>
  );
}

/** 펼친 보기의 오른쪽 — 정체와 자리, 담길 칸, 품목. */
function WideShot({ shot, tab, reading, done, picks, onChoose, on, link, stopOf }: {
  shot: MirrorShot; tab: MirrorTab; reading: boolean; done: Done | undefined; picks: string[];
  onChoose: (choiceId: string, restoring: boolean) => void;
  on: (key: string, sources: string[]) => boolean;
  link: (key: string, sources: string[], stop: boolean) => Record<string, unknown>;
  stopOf: (list: { key: string; sources: string[] }[]) => string | undefined;
}) {
  const cmp = shot.compareSeq;
  const cmpHead = 견줌(shot);
  const { verdict, tone, sub } = 판정(shot, done);
  const idSources = [...new Set(shot.fields.filter((f) => f.kind === "id").flatMap((f) => f.sources))];
  const places = shot.place.map((p) => ({ p, key: `p:${p.kind}`, sources: p.state === "here" ? idSources : p.source ? [p.source] : [] }));
  const placeStop = stopOf(places);
  const fields = shot.fields.map((f) => ({ f, key: `f:${f.column}`, sources: f.sources }));
  const fieldStop = stopOf(fields);
  const items = shot.items.map((it) => ({ it, key: `i:${it.line}`, sources: it.source ? [it.source] : [] }));
  const itemStop = stopOf(items);
  const itemsNote = shot.itemsAllRead ? `화면의 품목 표 ${shot.itemRows}행을 모두 읽었습니다`
    : shot.itemRows === 0 ? "화면에서 품목 표를 읽지 않았습니다" : "";

  return (
    <>
      <section aria-label="작업자료에서의 자리" className={`wide-card wide-id${reading ? " reading" : ""}`}>
        <div className="wide-id-head">
          <div className="dock-pills">
            <span className="kind-pill">{shot.kind}</span>
            <h3 className="num">{shot.number}</h3>
            <span className={`pill ${tone}`}>{verdict}</span>
          </div>
          <p className="dock-name">{shot.title}</p>
          {sub && <p className="soft small">{sub}</p>}
          <p className="soft tiny num">{reading ? "화면을 읽는 중" : tab.readAt ? `${이마(tab.readAt)}에 읽음` : ""}</p>
        </div>
        <div role="list" aria-label="접수에서 계약까지" className="wide-chain" onKeyDown={rove(true)}>
          {places.map(({ p, key, sources }, i) => (
            <div key={p.kind} className="wide-chain-step">
              {i > 0 && <span className="wide-chain-arrow" aria-hidden="true">{아이콘.arrow}</span>}
              <div role="listitem" className={`wide-place place-${p.state}${p.state === "here" && done ? " landed" : ""}${on(key, sources) ? " lit" : ""}`}
                {...link(key, sources, key === placeStop)}>
                <span className="place-kind">{p.kind}</span>
                {p.state !== "missing" && <b className="num clip">{p.number}</b>}
                <span className="soft tiny clip">{자리말(p)}</span>
                {p.state === "here" && (
                  <ol className="dock-rounds">
                    {shot.rounds.map((r) => {
                      const ghost = r.state === "ghost" && !done;
                      const landed = r.state === "ghost" && !!done;
                      return (
                        <li key={r.seq} className={`round ${ghost ? "ghost" : landed ? "landed" : r.state}`}>
                          <span className="strong">차수 {r.seq}</span>
                          <span className="num">{r.amount}</span>
                          <span className="round-note">{차수말(r, done)}</span>
                        </li>
                      );
                    })}
                  </ol>
                )}
              </div>
            </div>
          ))}
        </div>
      </section>

      <section aria-label="저장될 칸" className="wide-card">
        <div className="wide-card-head">
          <h3>저장될 칸</h3>
          <span className="soft tiny num">{shot.view} · 계약면 {shot.viewColumns}열 중 이 화면에서 읽는 {shot.fields.length}열</span>
        </div>
        <div className="wide-scroll">
          <div role="table" aria-label="저장될 칸" className="wide-table wide-fields" onKeyDown={rove(false)}>
            <div role="row" className="wide-th">
              <span role="columnheader">계약면의 열</span><span role="columnheader">수집 값</span>
              <span role="columnheader">{cmpHead || "비교"}</span>
            </div>
            {fields.map(({ f, key, sources }) => {
              const restoring = f.choiceId !== null && picks.includes(f.choiceId);
              return (
                <div role="row" key={key} className={`wide-tr${on(key, sources) ? " lit" : ""}`} {...link(key, sources, key === fieldStop)}>
                  <span role="cell" className="wide-col">
                    <span className="strong">{f.column}</span>
                    {f.from && <span className="soft tiny clip">화면 · {f.from}</span>}
                  </span>
                  <span role="cell" className="wide-val">
                    <span className={`num${f.kind === "changed" ? " changed" : f.kind === "override" && !restoring ? " soft" : ""}`}>{f.value}</span>
                    {f.raw && <span className="soft tiny num">화면 표기 {f.raw}</span>}
                  </span>
                  <span role="cell" className="wide-cmp">
                    {f.kind === "id" ? <span className="soft tiny">식별</span>
                      : f.kind === "same" ? <span className="soft small">같음</span>
                      : f.kind === "new" ? <span className="soft small">새로 담김</span>
                      : f.kind === "changed" ? <span className="wide-changed"><b>바뀜</b><s className="soft num">{f.old || "빈 값"}</s></span>
                      : (
                        <span className="wide-override">
                          <span className="dock-human">고친 값 · {restoring ? <><s>{f.human}</s> <b className="danger">지움</b></> : f.human}</span>
                          <span role="group" aria-label={`${f.column} — 고친 값을 둘지, 지우고 수집 원값을 쓸지`} className="dock-seg">
                            <button type="button" aria-pressed={!restoring} onClick={() => onChoose(f.choiceId!, false)}>고친 값 유지</button>
                            <button type="button" aria-pressed={restoring} onClick={() => onChoose(f.choiceId!, true)}>수집 원값으로 복원</button>
                          </span>
                        </span>
                      )}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
        {shot.rest > 0 && <p className="dock-rest soft tiny">나머지 {shot.rest}열 · 이 화면에서는 읽지 않음</p>}
      </section>

      <section aria-label="품목" className="wide-card">
        <div className="wide-card-head"><h3>품목</h3><span className="soft tiny">{itemsNote}</span></div>
        {items.length === 0 ? <p className="dock-rest soft tiny">품목이 없습니다.</p> : (
          <div className="wide-scroll">
            <div role="table" aria-label="품목" className="wide-table wide-items" onKeyDown={rove(false)}>
              <div role="row" className="wide-th">
                <span role="columnheader">순번</span><span role="columnheader">품명 · 규격</span>
                <span role="columnheader" className="right">수량</span><span role="columnheader">단위</span>
                <span role="columnheader" className="right">단가</span><span role="columnheader" className="right">금액</span>
                <span role="columnheader">{cmpHead || "비교"}</span>
              </div>
              {items.map(({ it, key, sources }) => {
                const change = 품목다름(it, cmp);
                return (
                  <div role="row" key={key} className={`wide-tr${on(key, sources) ? " lit" : ""}`} {...link(key, sources, key === itemStop)}>
                    <span role="cell" className="soft num">{it.line}</span>
                    <span role="cell" className="wide-col"><span className="clip">{it.name}</span>{it.spec && <span className="soft tiny clip">{it.spec}</span>}</span>
                    <span role="cell" className={`num right${change ? " changed" : ""}`}>{it.quantity}</span>
                    <span role="cell">{it.unit}</span>
                    <span role="cell" className="num right">{it.price}</span>
                    <span role="cell" className={`num right${change ? " changed" : ""}`}>{it.amount || it.price}</span>
                    <span role="cell" className={change ? "dock-change num" : "soft small"}>{change || "같음"}</span>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </section>
    </>
  );
}
