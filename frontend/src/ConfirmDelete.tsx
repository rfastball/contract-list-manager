import { Modal } from "./Modal";
import { useState } from "react";
import type { DeletionPlan, DeletionScope } from "./types";

type Props = {
  /** 계열 전체를 지울 때 무엇이 사라지는지. 차수 목록이 여기서 온다. */
  plan: DeletionPlan;
  /** 차수 하나만 지울 때의 셈. 차수가 하나뿐이면 오지 않는다. */
  one: DeletionPlan | null;
  onDelete: (scope: DeletionScope) => Promise<void>;
  onClose: () => void;
};

/**
 * 지우기 전에 <b>무엇이 사라지는지 세어 보인다</b>.
 *
 * <p>되돌릴 수 없는 일이다. 그런데 이 자료에서 "레코드 하나" 는 눈에 보이는 한 줄이 아니라
 * 차수 여럿과 딸린 줄 수십, 거기 매달린 사람의 메모와 확정한 링크다 — 세어 보이지 않으면
 * 사람은 자기가 무엇을 없애는지 모르는 채로 누른다.</p>
 *
 * <p><b>묘비를 두지 않는다.</b> 지운 뒤에 같은 자료를 다시 수집하면 다시 선다.</p>
 */
export function ConfirmDelete({ plan, one, onDelete, onClose }: Props) {
  /** 차수가 하나뿐이면 고를 것이 없다 — 그것을 지우는 것이 곧 계열을 지우는 것이다. */
  const [scope, setScope] = useState<DeletionScope>(one ? "seq" : "series");
  const [busy, setBusy] = useState(false);

  const shown = scope === "series" ? plan : (one ?? plan);
  const 공고 = plan.entityType === "notice";
  const 갈래 = { contract: "계약", notice: "공고", request: "접수" }[plan.entityType];

  const remove = async () => {
    setBusy(true);
    try {
      await onDelete(scope);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal label="자료 삭제" onClose={onClose}>
        <h2>{갈래} 삭제</h2>

        <p className="path">{plan.display}{plan.title && ` · ${plan.title}`}</p>

        {one && (
          <section className="field">
            <span className="label">삭제 범위</span>
            <div className="row">
              {([
                ["seq", `이 차수만 (${one.seq})`],
                ["series", `전 차수 (${plan.revisions.length})`],
              ] as const).map(([which, label]) => (
                <button
                  key={which}
                  className="chip"
                  aria-pressed={scope === which}
                  onClick={() => setScope(which)}
                >
                  {label}
                </button>
              ))}
            </div>
            <p className="hint">저장된 차수: {plan.revisions.join(" · ")}</p>
          </section>
        )}

        <section className="field">
          <span className="label">함께 삭제되는 자료</span>

          <ul className="tally">
            <li><span>하위 자료 행</span><b>{shown.childRows}</b></li>
            <li><span>수정한 셀</span><b>{shown.overrides}</b></li>
            {shown.seriesGoes && (
              <li><span>사용자 입력값</span><b>{shown.userFields}</b></li>
            )}
          </ul>

          {shown.seriesGoes && shown.linked && (
            <p className="hint">
              <b className="warn">
                {공고 ? "이 공고와 계약의 연결도 해제됩니다." : "확정한 공고 연결도 해제됩니다."}
              </b>
            </p>
          )}

          {!shown.seriesGoes && (
            <p className="hint">
              사용자 입력값과 공고 연결은 유지합니다.
              삭제 후에는 남은 차수 중 최신 자료를 표시합니다.
            </p>
          )}
        </section>

        <p className="hint">
          <b className="warn">되돌릴 수 없습니다.</b>
        </p>

        <div className="row end">
          <button className="action" onClick={onClose}>취소</button>
          <button className="action danger" onClick={() => void remove()} disabled={busy}>
            {busy ? "삭제 중…" : "삭제"}
          </button>
        </div>
    </Modal>
  );
}
