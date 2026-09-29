import { useState } from "react";
import type { EntityType, UserColumn } from "./types";

/** 표가 실제로 다르게 다루는 종류만 낸다. 하는 일이 없는 선택지를 내놓지 않는다. */
const KINDS = [
  { value: "text", label: "자유 입력" },
  { value: "choice", label: "고르기" },
] as const;

type Entity = EntityType;

/** 흐름 차례대로 — 계약이 맨 앞인 것은 사람이 가장 많이 적는 자리라서다. */
const ENTITIES: Entity[] = ["contract", "notice", "request"];

const 이름: Record<Entity, string> = { contract: "계약", notice: "공고", request: "접수" };

type Actions = {
  onAdd: (entityType: Entity, name: string, kind: string, choices: string) => Promise<void>;
  onUpdate: (
    entityType: Entity, fieldName: string, newName: string, kind: string, choices: string,
  ) => Promise<void>;
  onRemove: (entityType: Entity, fieldName: string) => Promise<void>;
  onMove: (entityType: Entity, fieldName: string, delta: number) => Promise<void>;
};

type Props = Actions & { columns: UserColumn[] };

/**
 * 손으로 채우는 열을 사람이 직접 세우고 고치는 자리.
 *
 * <p>미리 세워 주는 열은 진행상태와 메모 둘뿐이다. 사람마다 관리대장에 적는 것이 달라서,
 * 넷을 미리 깔아 두면 <b>쓰지 않는 열이 표를 넓히고</b> 무엇을 채워야 하는지 헷갈리게 한다.
 * 필요한 것은 스스로 세우는 편이 낫다.</p>
 *
 * <p><b>고치면 곧바로 반영된다.</b> 열이 바뀌면 계약면 뷰를 다시 지어야 하는데, 그것을
 * "저장" 단추 뒤로 미루면 화면과 DB 가 갈라져 있는 동안이 생긴다.</p>
 *
 * <p><b>열을 지워도 적어 둔 값은 남는다.</b> 열 하나에 백 건의 메모가 달려 있을 수 있어서,
 * 단추 한 번에 그것이 사라지면 되돌릴 길이 없다. 같은 이름으로 다시 세우면 되살아난다.</p>
 */
export function ColumnsEditor({ columns, ...actions }: Props) {
  return (
    <div className="columns">
      {ENTITIES.map((entity) => (
        <Group
          key={entity}
          entity={entity}
          columns={columns.filter((c) => c.entityType === entity)}
          {...actions}
        />
      ))}
    </div>
  );
}

function Group({ entity, columns, onAdd, ...rest }: Props & { entity: Entity }) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);

  const add = async () => {
    if (!name.trim() || busy) return;

    setBusy(true);
    try {
      await onAdd(entity, name, "text", "");
      setName("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="columns-group">
      <h3>{이름[entity]}</h3>

      {columns.length === 0 ? (
        <p className="hint">아직 열이 없습니다.</p>
      ) : (
        columns.map((c, i) => (
          <Row
            // 이름이 바뀌면 새로 태어나게 한다 — 안에 든 초안이 옛 이름을 붙들고 있지 않게.
            key={`${c.fieldName} ${c.kind}`}
            column={c}
            first={i === 0}
            last={i === columns.length - 1}
            {...rest}
          />
        ))
      )}

      <div className="row">
        <input
          className="text"
          value={name}
          placeholder="새 열 이름"
          maxLength={20}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") void add(); }}
        />
        <button className="action" disabled={!name.trim() || busy} onClick={() => void add()}>
          열 추가
        </button>
      </div>
    </section>
  );
}

type RowProps = Omit<Actions, "onAdd"> & {
  column: UserColumn;
  first: boolean;
  last: boolean;
};

function Row({ column, first, last, onUpdate, onRemove, onMove }: RowProps) {
  const [name, setName] = useState(column.fieldName);
  const [choices, setChoices] = useState(column.choices.join(", "));
  const [asking, setAsking] = useState(false);

  const entity = column.entityType;

  const save = (over: { name?: string; kind?: string; choices?: string }) =>
    void onUpdate(
      entity,
      column.fieldName,
      (over.name ?? name).trim() || column.fieldName,
      over.kind ?? column.kind,
      over.choices ?? choices,
    );

  return (
    <div className="columns-row">
      <input
        className="text"
        value={name}
        maxLength={20}
        aria-label={`${column.fieldName} 이름`}
        onChange={(e) => setName(e.target.value)}
        onBlur={() => { if (name.trim() !== column.fieldName) save({}); }}
        onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }}
      />

      <select
        value={KINDS.some((k) => k.value === column.kind) ? column.kind : "text"}
        aria-label={`${column.fieldName} 종류`}
        onChange={(e) => save({ kind: e.target.value })}
      >
        {KINDS.map((k) => (
          <option key={k.value} value={k.value}>{k.label}</option>
        ))}
      </select>

      {column.kind === "choice" ? (
        <input
          className="text"
          value={choices}
          placeholder="선택 항목 (쉼표로 구분)"
          aria-label={`${column.fieldName} 후보`}
          onChange={(e) => setChoices(e.target.value)}
          onBlur={() => { if (choices !== column.choices.join(", ")) save({}); }}
          onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }}
        />
      ) : (
        <span className="spacer" />
      )}

      {asking ? (
        <>
          <button className="action danger" onClick={() => void onRemove(entity, column.fieldName)}>
            열 삭제
          </button>
          <button className="action" onClick={() => setAsking(false)}>취소</button>
        </>
      ) : (
        <>
          <button
            className="action icon" disabled={first}
            aria-label={`${column.fieldName} 위로`}
            onClick={() => void onMove(entity, column.fieldName, -1)}
          >
            ↑
          </button>
          <button
            className="action icon" disabled={last}
            aria-label={`${column.fieldName} 아래로`}
            onClick={() => void onMove(entity, column.fieldName, 1)}
          >
            ↓
          </button>
          <button
            className="action icon"
            aria-label={`${column.fieldName} 삭제`}
            onClick={() => setAsking(true)}
          >
            ✕
          </button>
        </>
      )}
    </div>
  );
}
