import type { StatusReport } from "./types";

type Props = {
  report: StatusReport;
  /** 거르개를 그대로 올려 준다. 푸는 것은 바깥 한 자리의 몫이다. */
  onFilter: (거르개: string) => void;
};

/**
 * 뒤에서 센 것을 <b>오는 대로</b> 그린다.
 *
 * <p><b>배치를 지표에 맞춰 짜지 않는다.</b> 무엇이 몇 묶음으로 올지 이 화면은 알지 못하고,
 * 알아서도 안 된다 — 지표를 더하거나 빼는 일이 뒤의 배열 하나(<code>Status.Metrics</code>)를
 * 고치는 일이어야 하기 때문이다. 여기에 「단계」나 「쌓인 것」 같은 이름이 나타나는 순간
 * 그 성질이 무너진다.</p>
 *
 * <p>깔때기도 그림도 색도 없다. 지금 필요한 것은 <b>뒤에서 센 것이 앞에 뜬다</b>를 눈으로
 * 보는 것뿐이고, 지표 자체가 아직 가상이라 꾸미는 것이 이르다.</p>
 *
 * <p>거르개가 달린 줄만 누를 수 있다. 누르면 계획 탭이 그 조건으로 걸린다 — 「17건」 을 보고
 * 그 17줄이 무엇인지 곧바로 열어 보는 것이 이 화면의 유일한 쓸모다.</p>
 */
export function Status({ report, onFilter }: Props) {
  if (report.groups.length === 0)
    return <div className="empty-state">표시할 현황이 없습니다.</div>;

  return (
    <div className="status-board">
      {report.groups.map((group) => (
        <section className="status-group" key={group.이름} aria-label={group.이름}>
          <h3>{group.이름}</h3>

          <ul>
            {group.cells.map((cell) => (
              <li key={cell.이름}>
                {cell.거르개 !== null ? (
                  <button
                    className="status-open"
                    title={`계획 탭을 ${cell.거르개} 로 걸러 봅니다`}
                    aria-label={`${cell.이름} 만 보기`}
                    onClick={() => onFilter(cell.거르개!)}
                  >
                    <span className="status-name">{cell.이름}</span>
                    <span className="status-count">{cell.수.toLocaleString()}</span>
                    <span aria-hidden="true">▸</span>
                  </button>
                ) : <><span className="status-name">{cell.이름}</span><span className="status-count">{cell.수.toLocaleString()}</span></>}
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
