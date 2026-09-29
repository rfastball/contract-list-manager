import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { Outline } from "./Outline";
import type {
  Outline as Tree, OutlineContract, OutlineNotice, OutlineNoticeGroup, OutlineRequest,
} from "./types";

function 계약(number: string, title: string, 명세: number, 차수 = ["00"]): OutlineContract {
  return {
    key: number.slice(0, -2),
    number,
    title,
    contractedOn: "2026/07/22",
    amount: "164,872,340",
    counterparty: "㈜한국분석기기",
    demandAgency: "가람군수지원단",
    revisions: 차수,
    items: Array.from({ length: 명세 }, (_, i) => ({
      lineNo: i + 1,
      name: `품목${i + 1}`,
      specification: "별첨 규격서에 따름",
      quantity: "1",
      unit: "대",
      unitPrice: "82,436,170",
      amount: "82,436,170",
    })),
  };
}

function 접수(base: string, title: string, 요구번호: string[]): OutlineRequest {
  return {
    key: base,
    number: `${base}-000`,
    title,
    receivedOn: "2026/06/09",
    goodsAmount: "181,725,200",
    budgetAmount: "183,408,620",
    demandAgency: "한별군수지원단",
    requestNumbers: 요구번호,
    revisions: ["000"],
  };
}

/** 갈리지 않은 건. 공고 하나가 그대로 제 건이라 대체된 것이 없다. */
function 건(notice: OutlineNotice): OutlineNoticeGroup {
  return { groupBase: notice.key, current: notice, superseded: [] };
}

function 공고(
  base: string, seq: string, title: string, postedAt: string, 차수: string[],
): OutlineNotice {
  return {
    key: base,
    number: `${base}-${seq}`,
    title,
    postedAt,
    agency: "국방부조달본부",
    estimatedPrice: "187,340,098",
    revisions: 차수,
  };
}

/** 여덟 줄짜리 접수. 조달요구번호를 몇 개만 펴고 접는지 여기서 본다. */
const 여덟 = Array.from({ length: 8 }, (_, i) => `MPKPLA269102${86 + i}`);

const tree: Tree = {
  chains: [
    // 세 칸이 다 찬 사슬. 계약은 하나뿐이라 갈라지지 않는다.
    {
      key: "MPKPLA26910301",
      request: 접수("MPKPLA26910301", "수질측정기 요청", ["MPKPLA26910301"]),
      notice: 건(공고("R26BK09017075", "001", "수질측정기 구매",
                     "2026/07/02 16:59:28", ["000", "001"])),
      contracts: [계약("R26TA0911050700", "수질측정기 조달", 2)],
    },
    // 계약이 둘. 끝 칸만 갈라진다.
    {
      key: "R26BK09017080",
      request: null,
      notice: 건(공고("R26BK09017080", "000", "가람 절단기 구매",
                     "2026/07/03 10:00:00", ["000"])),
      contracts: [
        계약("R26TA0911050800", "가람 절단기 조달", 0),
        계약("R26TA0911050900", "가람 절단기 조달 (2차분)", 0),
      ],
    },
    // 접수만 들어온 사슬. 공고도 계약도 아직 없다.
    {
      key: "MPKPLA26910286",
      request: 접수("MPKPLA26910286", "26년한별윈치8종구매", 여덟),
      notice: null,
      contracts: [],
    },
    // 어디에도 매달리지 못한 계약.
    {
      key: "R26TA09990001",
      request: null,
      notice: null,
      contracts: [계약("R26TA0999000100", "한별 소나 부품 구매", 0)],
    },
    // 취소 뒤 재공고로 본번호가 갈린 건. 사슬은 하나이고 본문에는 재공고가 선다.
    {
      key: "MBKLMH26930006/R26BK09011054",
      request: 접수("MBKLMH26930006", "가스성분분석기 요청", ["MBKLMH26930006"]),
      notice: {
        groupBase: "R26BK09011054",
        current: 공고("R26BK09012082", "000", "가스성분분석기 구매",
                    "2026/07/25 09:30:00", ["000"]),
        superseded: [
          공고("R26BK09011054", "001", "가스성분분석기 구매",
              "2026/07/09 14:20:00", ["000", "001"]),
        ],
      },
      contracts: [계약("R26TA0908046900", "가스성분분석기 조달", 0)],
    },
  ],
};

// jsdom 에는 없다. 격자가 커서를 따라 굴리는 자리에서 걸려 넘어지지 않게 둔다.
beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
});

afterEach(cleanup);

const draw = () => render(<Outline tree={tree} />);

/** 사슬 한 장. 격자의 <b>줄</b>이라 role 은 row 다 — 이름은 그 사슬을 부르는 이름 그대로. */
const 사슬 = (name: string) => screen.getByRole("row", { name });

const 격자 = () => screen.getByRole("grid");

/**
 * 지금 커서가 선 칸. <b>DOM 초점이 아니다</b> — 초점은 격자가 쥐고 있고, 커서가 어디인지는
 * <code>aria-activedescendant</code> 하나가 말한다. 낭독기가 보는 것을 시험도 그대로 본다.
 */
function 커서(): HTMLElement | null {
  const id = 격자().getAttribute("aria-activedescendant");
  return id === null ? null : document.getElementById(id);
}

describe("Outline", () => {
  /**
   * 이 화면이 있는 까닭.
   *
   * 품목 표에서는 명세가 두 줄이면 계약번호가 두 번 찍혔다. 되풀이지만 보는 사람에게는
   * 자료가 겹쳐 들어간 것처럼 보인다. 여기서는 계약이 한 번만 서야 한다.
   */
  it("명세가 여러 줄이어도 계약번호는 한 번만 찍힌다", async () => {
    const user = userEvent.setup();
    draw();

    expect(screen.getAllByText("R26TA0911050700")).toHaveLength(1);

    // 펼쳐서 명세를 꺼내도 계약 줄은 그대로 하나다.
    await user.click(screen.getByRole("button", { name: /품목 2/ }));

    expect(screen.getByText("품목1")).toBeTruthy();
    expect(screen.getByText("품목2")).toBeTruthy();
    expect(screen.getAllByText("R26TA0911050700")).toHaveLength(1);
  });

  /**
   * 조달의 기본은 1:1:1 이다. 그런데 하나뿐인 계약을 목록처럼 그리면 여럿 달린 것처럼
   * 보이고, 정말 여럿일 때와 구별이 가지 않는다.
   */
  it("계약이 하나면 갈라지지 않고 둘이면 갈라진다", () => {
    draw();

    const 하나 = 사슬("수질측정기 요청");
    expect(within(하나).getAllByText("계약")).toHaveLength(1);
    expect(within(하나).queryByText(/계약 \d+건/)).toBeNull();

    const 둘 = 사슬("가람 절단기 구매");
    expect(within(둘).getAllByText("계약")).toHaveLength(2);
    expect(within(둘).getByText("계약 2건")).toBeTruthy();
  });

  /** 접수서만 먼저 들어오는 일이 흔하다. 빠진 자리가 사라지면 무엇이 없는지 안 보인다. */
  it("공고 없이 접수만 있는 사슬도 선다", () => {
    draw();

    const 사슬하나 = 사슬("26년한별윈치8종구매");
    expect(within(사슬하나).getByText("26년한별윈치8종구매")).toBeTruthy();

    // 공고 칸과 계약 칸이 자리를 지킨다.
    expect(within(사슬하나).getByText("공고")).toBeTruthy();
    expect(within(사슬하나).getAllByText("(아직 없음)")).toHaveLength(2);
  });

  /** 계약서만 먼저 들어오는 일도 흔하다. 안 보이면 넣은 사람이 넣지 않은 줄 안다. */
  it("어디에도 매달리지 못한 계약도 남는다", () => {
    draw();

    const 홀로 = 사슬("한별 소나 부품 구매");
    expect(within(홀로).getByText("한별 소나 부품 구매")).toBeTruthy();
    expect(within(홀로).getAllByText("(아직 없음)")).toHaveLength(2);
  });

  /** 여덟 개가 다 서면 접수 칸이 다른 두 칸을 밀어낸다. */
  it("조달요구번호는 몇 개만 서고 나머지는 셈으로 접힌다", async () => {
    const user = userEvent.setup();
    draw();

    const 사슬하나 = 사슬("26년한별윈치8종구매");
    expect(within(사슬하나).getByText("MPKPLA26910286")).toBeTruthy();
    expect(within(사슬하나).queryByText("MPKPLA26910293")).toBeNull();

    await user.click(within(사슬하나).getByRole("button", { name: "외 5건" }));

    expect(within(사슬하나).getByText("MPKPLA26910293")).toBeTruthy();
  });

  /** 차수 셋이 세 줄로 서면 건수가 부풀어 보인다. 본문에는 최신 하나만 선다. */
  it("문서 개수와 차수 내역을 펼친다", async () => {
    const user = userEvent.setup();
    draw();

    expect(screen.getByText("R26BK09017075-001")).toBeTruthy();
    expect(screen.queryByText("R26BK09017075-000")).toBeNull();
    const summary = within(사슬("수질측정기 요청")).getByText("문서 2개");
    await user.click(summary);
    expect(summary.closest("details")?.open).toBe(true);
    expect(screen.getByText("차수 R26BK09017075-000 · R26BK09017075-001")).toBeTruthy();
  });

  /**
   * 취소되고 재채번된 공고는 차수가 아니라 <b>본번호</b>가 갈린다. 공고마다 칸을 세우면
   * 접수도 계약도 하나인 조달 건이 둘로 서고, 그중 접수와 계약을 안고 서는 것이 <b>취소된
   * 원공고</b>이며 살아 있는 재공고는 빈손으로 옆에 선다 — 어느 줄에도 틀린 값이 없어
   * 수가 늘어난 것으로만 나타난다.
   */
  it("취소·재공고로 갈린 건도 사슬 하나로 서고 본문에는 현행이 선다", () => {
    draw();

    const 하나 = 사슬("가스성분분석기 요청");

    // 본문에 서는 것은 살아 있는 재공고다. 취소된 원공고는 이름표의 장수로만 남는다.
    expect(within(하나).getByText("R26BK09012082-000")).toBeTruthy();
    expect(screen.queryByText("R26BK09011054-001")).toBeNull();
    // 이름표는 공고번호째로 적는다 — 차수만 적으면 갈린 두 본번호의 `000` 이 두 번 나온다.
    expect(within(하나).getByText(
      "차수 R26BK09012082-000 · R26BK09011054-000 · R26BK09011054-001")).toBeTruthy();

    // 접수와 계약이 그 한 사슬에 붙는다. 비어 있는 칸은 하나도 없다.
    expect(within(하나).getByText("MBKLMH26930006-000")).toBeTruthy();
    expect(within(하나).getByText("R26TA0908046900")).toBeTruthy();
    expect(within(하나).queryByText("(아직 없음)")).toBeNull();
  });

  /**
   * 두 본번호가 서로의 최신 차수를 가리키면 <b>대체되지 않은 것이 하나도 남지 않는다</b>.
   * 그때도 공고는 들어와 있으므로 빈 칸으로 떨어뜨리면 안 된다 — 아직 안 들어온 것과
   * 구별이 가지 않아, 넣은 사람이 넣지 않은 줄 안다.
   */
  it("현행이 정해지지 않은 건은 빈 칸이 아니라 (현행 확인 필요) 으로 선다", () => {
    render(
      <Outline
        tree={{
          chains: [{
            key: "MBKLMH26930009/R26BK09011054",
            request: 접수("MBKLMH26930009", "서로 가리키는 건", ["MBKLMH26930009"]),
            notice: {
              groupBase: "R26BK09011054",
              current: null,
              superseded: [공고("R26BK09011054", "001", "서로 가리키는 공고",
                              "2026/07/09 14:20:00", ["000", "001"])],
            },
            contracts: [],
          }],
        }}
      />,
    );

    expect(screen.getByText("(현행 확인 필요)")).toBeTruthy();

    // 빈 칸으로 서는 것은 계약 하나뿐이다 — 공고 칸은 제 몫으로 서 있다.
    expect(screen.getAllByText("(아직 없음)")).toHaveLength(1);
  });

  it("찾으면 걸리는 것만 남고, 접혀 있어도 펴진다", async () => {
    const user = userEvent.setup();
    draw();

    await user.type(screen.getByRole("searchbox"), "소나");

    expect(screen.queryByText("수질측정기 구매")).toBeNull();
    expect(screen.getByText("한별 소나 부품 구매")).toBeTruthy();

  });

  it("공고가 걸리면 그 사슬의 계약이 모두 따라 나온다", async () => {
    const user = userEvent.setup();
    draw();

    await user.type(screen.getByRole("searchbox"), "수질측정");

    expect(within(사슬("수질측정기 요청")).getByText("수질측정기 조달")).toBeTruthy();
    expect(screen.getByRole("button", { name: /품목 2/ }).getAttribute("aria-expanded")).toBe("true");
    await user.click(screen.getByRole("button", { name: "모두 접기" }));
    expect(screen.getByRole("button", { name: /품목 2/ }).getAttribute("aria-expanded")).toBe("false");
    await user.click(screen.getByRole("button", { name: /품목 2/ }));
    expect(screen.getByRole("button", { name: /품목 2/ }).getAttribute("aria-expanded")).toBe("true");
  });

  it("찾은 것이 없을 때 검색을 지우면 전체 건으로 돌아온다", async () => {
    const user = userEvent.setup();
    draw();
    const total = screen.getAllByRole("row").length;
    await user.type(screen.getByRole("searchbox"), "없는건명zz");
    expect(screen.queryAllByRole("row")).toHaveLength(0);
    expect(screen.getByRole("heading", { name: "검색 결과가 없습니다" })).toBeTruthy();
    격자().focus();
    await user.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "검색 지우기" }));
    await user.keyboard(" ");
    expect(screen.getAllByRole("row")).toHaveLength(total);
    expect(document.activeElement).toBe(screen.getByRole("searchbox"));
  });

  /** 조달요구번호로도 찾을 수 있어야 한다 — 사람이 손에 들고 있는 번호가 그것이다. */
  it("조달요구번호로 접수를 찾는다", async () => {
    const user = userEvent.setup();
    draw();

    await user.type(screen.getByRole("searchbox"), "MPKPLA26910290");

    expect(screen.getByText("26년한별윈치8종구매")).toBeTruthy();
    expect(screen.queryByText("한별 소나 부품 구매")).toBeNull();
  });

  // ── 손 ──────────────────────────────────────────────
  // 옆의 표 보기가 엑셀 키를 쥐고 있다. 같은 자료의 두 눈이 서로 다른 손을 요구하면 탭을
  // 옮길 때마다 다시 배워야 하므로, 여기서 보는 것은 표와 같은 손인지다.

  /** 초점이 들어왔는데 커서가 어디에도 없으면, 화살표를 눌러 봐야 무엇이 움직였는지 모른다. */
  it("격자에 초점이 가면 첫 사슬 첫 칸에 커서가 선다", () => {
    draw();
    격자().focus();

    const 칸 = 커서();
    expect(칸).not.toBeNull();
    expect(칸!.classList.contains("cur")).toBe(true);
    expect(within(칸!).getByText("MPKPLA26910301-000")).toBeTruthy();
  });

  it("↑↓ 는 사슬을 옮긴다", async () => {
    const user = userEvent.setup();
    draw();
    격자().focus();

    await user.keyboard("{ArrowDown}");
    expect(사슬("가람 절단기 구매").contains(커서())).toBe(true);

    await user.keyboard("{ArrowUp}");
    expect(사슬("수질측정기 요청").contains(커서())).toBe(true);
  });

  /** 계약이 둘인 줄은 칸이 넷이다 — 세 칸으로 세면 둘째 계약에 손이 닿지 않는다. */
  it("←→ 는 사슬 안에서 칸을 옮기고, 계약이 둘이면 한 칸 더 간다", async () => {
    const user = userEvent.setup();
    draw();
    격자().focus();

    // 계약이 하나인 사슬은 셋째 칸에서 멈춘다.
    await user.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
    expect(within(커서()!).getByText("R26TA0911050700")).toBeTruthy();

    // 계약이 둘인 사슬에서는 그 자리에서 오른쪽으로 한 칸이 더 있다.
    await user.keyboard("{ArrowDown}");
    expect(within(커서()!).getByText("가람 절단기 조달")).toBeTruthy();

    await user.keyboard("{ArrowRight}");
    expect(within(커서()!).getByText("가람 절단기 조달 (2차분)")).toBeTruthy();
  });

  /** 넷째 칸에 선 채로 세 칸짜리 줄에 내려가면, 물리지 않는 한 커서가 없는 자리를 짚는다. */
  it("칸이 적은 사슬로 내려가면 커서가 마지막 칸으로 물린다", async () => {
    const user = userEvent.setup();
    draw();
    격자().focus();

    await user.keyboard("{ArrowDown}{ArrowRight}{ArrowRight}{ArrowRight}");
    expect(within(커서()!).getByText("가람 절단기 조달 (2차분)")).toBeTruthy();

    await user.keyboard("{ArrowDown}");
    expect(사슬("26년한별윈치8종구매").contains(커서())).toBe(true);
    expect(within(커서()!).getByText("계약")).toBeTruthy();
  });

  it("Space 는 지금 칸의 접이를 여닫는다", async () => {
    const user = userEvent.setup();
    draw();
    격자().focus();

    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(screen.queryByText("품목1")).toBeNull();

    await user.keyboard(" ");
    expect(screen.getByText("품목1")).toBeTruthy();

    await user.keyboard(" ");
    expect(screen.queryByText("품목1")).toBeNull();
  });

  /**
   * 격자 안의 위젯은 격자의 키로 닿는다. Tab 차례에 세우면 사슬마다 단추가 하나씩 서서,
   * 넷째 사슬의 품목을 펴려고 앞의 모든 단추를 밟아야 한다.
   */
  it("접이 단추는 Tab 차례에 서지 않는다", () => {
    draw();

    const 접이들 = screen.getAllByRole("button", { name: /품목|외 \d+건/ });
    expect(접이들.length).toBeGreaterThan(0);
    for (const 단추 of 접이들) expect(단추.tabIndex).toBe(-1);
  });

  /** 지우기만 하고 초점을 두고 오면 손이 찾기 칸에 갇혀 화살표가 아무 데도 닿지 않는다. */
  it("찾기 칸에서 Esc 를 누르면 질의가 지워지고 격자로 초점이 돌아온다", async () => {
    const user = userEvent.setup();
    draw();

    const 찾기 = screen.getByRole("searchbox");
    await user.type(찾기, "소나");
    expect(screen.queryByText("수질측정기 구매")).toBeNull();

    await user.type(찾기, "{Escape}");

    expect((찾기 as HTMLInputElement).value).toBe("");
    expect(document.activeElement).toBe(격자());
    expect(screen.getByText("수질측정기 구매")).toBeTruthy();
  });

  /** 걸러 사슬이 줄면 커서가 사라진 줄을 가리킬 수 있다. 표가 커서를 안으로 당기는 그 자리다. */
  it("걸러 사슬이 줄어도 커서는 없는 줄을 가리키지 않는다", async () => {
    const user = userEvent.setup();
    draw();
    격자().focus();

    await user.keyboard("{Control>}{End}{/Control}");
    expect(사슬("가스성분분석기 요청").contains(커서())).toBe(true);

    await user.type(screen.getByRole("searchbox"), "소나");

    expect(커서()).not.toBeNull();
    expect(사슬("한별 소나 부품 구매").contains(커서())).toBe(true);
  });
});
