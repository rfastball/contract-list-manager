import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";
import { App } from "../App";

/** 문(features.ts)을 닫으면 사이드바에 「맞춤 뷰」 가 서지 않는다 — 맞춤 뷰를 다시 걷어야 할 때 이 깃발 하나로 걷힌다. */
vi.mock("../features", () => ({ USER_VIEWS_ENABLED: false }));

beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});

afterEach(cleanup);

it("깃발이 꺼져 있으면 사이드바에 「맞춤 뷰」 가 서지 않는다", async () => {
  render(<App />);
  expect(await screen.findByRole("tab", { name: "대시보드" })).toBeTruthy();
  expect(screen.queryByRole("navigation", { name: "뷰" })).toBeNull();
  expect(screen.queryByRole("button", { name: "뷰 만들기" })).toBeNull();
});
