import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LinkPanel } from "./LinkPanel";
import type { Candidate, LinkFacet, LinkWork } from "./types";

afterEach(cleanup);

function candidate(contractKey: string, noticeKey: string, over: Partial<Candidate> = {}): Candidate {
  return {
    contractKey,
    contractTitle: "26년 가람 절단기 구매",
    noticeKey,
    noticeTitle: "26년 가람 절단기 구매",
    confidence: 0.9,
    reason: "건명 일치",
    noticeContractCount: 0,
    titleMatched: true,
    blocker: "같은 이름 공고가 2건입니다",
    facets: [
      { name: "건명", contract: "26년 가람 절단기 구매", notice: "26년 가람 절단기 구매", agrees: true },
      { name: "수요기관", contract: "가람군수지원단", notice: "한별군수지원단", agrees: false },
      { name: "계약일 / 게시일", contract: "", notice: "2026-07-03", agrees: null },
    ],
    ...over,
  };
}

const 견줌: LinkFacet[] = [
  { name: "건명", contract: "26년 가람 절단기 구매", notice: "26년 가람 절단기 구매", agrees: true },
  { name: "수요기관", contract: "가람군수지원단", notice: "가람군수지원단", agrees: true },
];

const work: LinkWork = {
  contracts: [
    { key: "계약-A", title: "26년 가람 절단기 구매", candidateCount: 2, noticeKey: null, noticeTitle: "", decidedBy: null },
    { key: "계약-B", title: "26년 누림 분석장비 구매", candidateCount: 1, noticeKey: null, noticeTitle: "", decidedBy: null },
    { key: "계약-C", title: "26년 한별 소나 부품 구매", candidateCount: 0, noticeKey: null, noticeTitle: "", decidedBy: null },
    {
      key: "계약-D", title: "26년 한별 공압식승강판 20톤 구매", candidateCount: 0,
      noticeKey: "공고-9", noticeTitle: "26년 한별 공압식승강판 20톤 구매", decidedBy: "auto",
    },
  ],
  candidates: [
    candidate("계약-A", "공고-1"),
    candidate("계약-A", "공고-2"),
    candidate("계약-B", "공고-3", { titleMatched: false, confidence: 0.62, blocker: null }),
  ],
  notices: [
    { key: "공고-1", title: "26년 가람 절단기 구매", postedAt: "2026-07-02", linked: 0 },
    { key: "공고-9", title: "26년 한별 공압식승강판 20톤 구매", postedAt: "2026-06-29", linked: 1 },
    { key: "공고-7", title: "26년 가람 방수포 구매", postedAt: "2026-06-27", linked: 0 },
  ],
};

type Hooks = {
  onConfirm: (c: Candidate) => void;
  onReject: (c: Candidate) => void;
  onUnlink: (contractKey: string) => void;
  onCompare: (contractKey: string, noticeKey: string) => Promise<LinkFacet[]>;
};

function draw(over: Partial<Hooks> = {}, initialKey: string | null = null) {
  const onConfirm = over.onConfirm ?? vi.fn();
  const onReject = over.onReject ?? vi.fn();
  const onUnlink = over.onUnlink ?? vi.fn();
  const onCompare = over.onCompare ?? vi.fn().mockResolvedValue(견줌);

  render(
    <LinkPanel
      initialKey={initialKey}
      work={work}
      onConfirm={onConfirm}
      onReject={onReject}
      onUnlink={onUnlink}
      onCompare={onCompare}
    />,
  );

  return { onConfirm, onReject, onUnlink, onCompare };
}

describe("LinkPanel", () => {
  /**
   * 이 화면이 지켜야 할 약속.
   *
   * 후보 목록만 그리면, 후보를 모두 물리친 계약이 화면에서 통째로 사라진다.
   * 그러고 나면 공고와 연결되지 않은 채로 남았다는 사실을 어디서도 볼 수 없다.
   */
  it("후보가 하나도 없는 계약도 목록에 남는다", () => {
    draw();

    const list = screen.getByRole("listbox", { name: "계약 목록" });
    expect(within(list).getByText("26년 한별 소나 부품 구매")).toBeTruthy();
    expect(within(list).getByText(/계약-C · 후보 없음/)).toBeTruthy();
  });

  it("후보가 없는 계약을 고르면 그렇다고 알린다", async () => {
    const user = userEvent.setup();
    draw();

    await user.click(screen.getByText("26년 한별 소나 부품 구매"));

    expect(screen.getByText(/연결할 공고 후보가 없습니다/)).toBeTruthy();
  });

  it("전달받은 계약이 없어도 다른 계약을 자동 선택하지 않고 직접 선택한 후보만 보인다", async () => {
    const user = userEvent.setup();
    draw({}, "사라진-계약");
    expect(screen.getByText(/사라진-계약 자료가 목록에 없습니다/)).toBeTruthy();
    expect(screen.queryByRole("option", { selected: true })).toBeNull();
    await user.click(within(screen.getByRole("listbox")).getByText("26년 가람 절단기 구매"));

    // 직접 고른 계약의 후보만 보인다.
    expect(screen.getByText("공고-1")).toBeTruthy();
    expect(screen.getByText("공고-2")).toBeTruthy();
    expect(screen.queryByText("공고-3")).toBeNull();

    await user.click(screen.getByText("26년 누림 분석장비 구매"));

    expect(screen.getByText("공고-3")).toBeTruthy();
    expect(screen.queryByText("공고-1")).toBeNull();
  });

  /** 점수 한 줄이 아니라 항목마다 나란히 놓아야 사람이 3초에 판단한다. */
  it("견주기 표가 같은 것과 다른 것을 갈라 보여 준다", () => {
    draw();

    const rows = screen.getAllByRole("row");
    const 수요기관 = rows.find((r) => within(r).queryByText("수요기관"));
    const 계약일 = rows.find((r) => within(r).queryByText("계약일 / 게시일"));

    expect(수요기관?.className).toBe("differ");
    expect(within(수요기관!).getByText("한별군수지원단")).toBeTruthy();

    // 한쪽 값이 없으면 어긋난 것이 아니라 판정할 수 없는 것이다.
    expect(계약일?.className).toBe("unknown");
  });

  it("자동으로 연결하지 않은 까닭을 보여 준다", () => {
    draw();
    expect(screen.getAllByText(/자동으로 연결하지 않았습니다\. 같은 이름 공고가 2건입니다/))
      .toHaveLength(2);
  });

  it("연결과 아님이 그 후보를 넘긴다", async () => {
    const user = userEvent.setup();
    const { onConfirm, onReject } = draw();

    const card = screen.getByText("공고-2").closest(".card")!;
    await user.click(within(card as HTMLElement).getByRole("button", { name: "연결" }));
    await user.click(within(card as HTMLElement).getByRole("button", { name: "후보 제외" }));

    expect(onConfirm).toHaveBeenCalledWith(expect.objectContaining({ noticeKey: "공고-2" }));
    expect(onReject).toHaveBeenCalledWith(expect.objectContaining({ noticeKey: "공고-2" }));
  });

  /**
   * 이 화면의 기본은 <b>아직 할 일</b> 쪽이다. 이어진 것까지 함께 세우면 손볼 줄이
   * 이어진 줄에 묻혀, 지금까지 일하던 흐름이 그대로 이어지지 않는다.
   */
  it("기본은 미연결만 보인다", () => {
    draw();

    const list = screen.getByRole("listbox", { name: "계약 목록" });
    expect(within(list).getAllByRole("option")).toHaveLength(3);
    expect(within(list).queryByText("26년 한별 공압식승강판 20톤 구매")).toBeNull();

    expect(screen.getByRole("button", { name: "미연결 3" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "연결됨 1" }).getAttribute("aria-pressed")).toBe("false");
  });

  /** 잘못 이어진 것을 되돌리는 길. 그 줄이 서지 않으면 창에서는 끊을 자리가 없다. */
  it("연결됨으로 바꾸면 이어진 계약과 연결 해제 단추가 나온다", async () => {
    const user = userEvent.setup();
    const { onUnlink } = draw();

    await user.click(screen.getByRole("button", { name: "연결됨 1" }));

    const list = screen.getByRole("listbox", { name: "계약 목록" });
    expect(within(list).getByText(/계약-D ← 공고-9/)).toBeTruthy();

    expect(await screen.findByText("연결된 공고")).toBeTruthy();
    expect(screen.getByText("자동 연결")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "연결 해제" }));
    expect(onUnlink).toHaveBeenCalledWith("계약-D");
  });

  /** 추천이 못 찾은 짝으로 나가는 길. 건명이 전혀 달라도 번호를 알면 고를 수 있어야 한다. */
  it("찾기 칸에 공고번호를 치면 추천에 없던 공고가 나온다", async () => {
    const user = userEvent.setup();
    draw();

    // 치기 전에는 목록을 펴지 않는다 — 후보 카드가 공고 전부에 묻히면 안 된다.
    expect(screen.queryByRole("listbox", { name: "직접 찾은 공고" })).toBeNull();

    await user.type(screen.getByPlaceholderText("공고번호·건명으로 찾기"), "공고-7");

    const found = screen.getByRole("listbox", { name: "직접 찾은 공고" });
    expect(within(found).getByText("26년 가람 방수포 구매")).toBeTruthy();
    expect(within(found).queryByText("26년 한별 공압식승강판 20톤 구매")).toBeNull();
  });

  it("직접 고른 공고를 이으면 그 짝으로 확정된다", async () => {
    const user = userEvent.setup();
    const { onConfirm, onCompare } = draw();

    await user.type(screen.getByPlaceholderText("공고번호·건명으로 찾기"), "방수포");
    await user.click(screen.getByText("26년 가람 방수포 구매"));

    // 근거 없이 잇게 두지 않는다 — 후보 카드와 같은 견주기 표를 편 뒤에 잇는다.
    expect(onCompare).toHaveBeenCalledWith("계약-A", "공고-7");

    const card = (await screen.findByText("공고-7")).closest(".card")!;
    const table = await within(card as HTMLElement).findByRole("table");
    expect(within(table).getByText("수요기관")).toBeTruthy();

    await user.click(within(card as HTMLElement).getByRole("button", { name: "연결" }));

    expect(onConfirm).toHaveBeenCalledWith(
      expect.objectContaining({ contractKey: "계약-A", noticeKey: "공고-7" }));
  });

  it("화살표로 계약을 옮겨 간다", async () => {
    const user = userEvent.setup();
    draw();

    const list = screen.getByRole("listbox", { name: "계약 목록" });
    await user.click(within(list).getByText("26년 가람 절단기 구매"));
    await user.keyboard("{ArrowDown}");

    expect(screen.getByText("공고-3")).toBeTruthy();
  });
});
