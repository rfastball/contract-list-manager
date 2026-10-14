import { useMemo } from "react";
import type { FlowEnv } from "../flow/ast";
import { CodeEditor } from "./CodeEditor";
import { flowExtensions } from "./flowLanguage";
import { sqlExtensions, sqlReadOnly } from "./sqlLanguage";

/**
 * 코드 편집기 둘. CodeMirror 는 크므로 이 모듈째로 늦게 불러온다 — 흐름식·SQL 탭을 열 때만 실린다.
 */
type Common = { value: string; onChange?: (value: string) => void; readOnly?: boolean; describedBy?: string; placeholder?: string };

export function FlowCodeEditor({ env, ...rest }: Common & { env: FlowEnv }) {
  const extensions = useMemo(() => flowExtensions(env), [env]);
  return <CodeEditor label="흐름식" extensions={extensions} {...rest} />;
}

export function SqlCodeEditor({ label, schema, view, outer, onIssues, ...rest }: Common & {
  label: string;
  /** 없으면 하이라이트만 하는 읽기 전용 보기. */
  schema?: Record<string, string[]>;
  view?: string;
  /** 바깥 행의 별칭 → 계약면 뷰. 없으면 현재(view) 하나. 맞춤 뷰는 접수·공고·계약. */
  outer?: Record<string, string>;
  onIssues?: (n: number) => void;
}) {
  const extensions = useMemo(() => schema ? sqlExtensions(schema, view ?? "", onIssues, outer) : sqlReadOnly, [schema, view, onIssues, outer]);
  return <CodeEditor label={label} extensions={extensions} {...rest} />;
}
