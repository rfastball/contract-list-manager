// @vitest-environment node
import { describe, expect, it } from "vitest";
import { 과와, 은는, 을를, 이가, 받침 } from "./josa";

describe("받침에 맞는 조사", () => {
  it("연결 낱말·동사 뒤에 맞는 조사를 붙인다", () => {
    expect(은는("연결공고")).toBe("연결공고는");
    expect(은는("연결계약")).toBe("연결계약은");
    expect(은는("잇기")).toBe("잇기는");
    expect(은는("정렬")).toBe("정렬은");
    expect(을를("‘품명 모음’")).toBe("‘품명 모음’을");
  });
  it("숫자는 읽는 소리로 친다", () => {
    expect(받침("3")).toBe("ㅇ");
    expect(받침("1")).toBe("ㄹ");
    expect(받침("2")).toBe("");
  });
  it("브라우저 이름은 Chrome 만 받침이 있다", () => {
    expect(이가("Chrome")).toBe("Chrome이");
    expect(이가("Edge")).toBe("Edge가");
    expect(이가("기타 브라우저")).toBe("기타 브라우저가");
    expect(과와("차수 3")).toBe("차수 3과");
  });
});
