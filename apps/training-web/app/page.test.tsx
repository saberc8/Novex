import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import Page from "./page";

describe("Training home page", () => {
  it("renders the customer-facing training workbench sections", () => {
    render(<Page />);

    expect(screen.getByRole("heading", { name: "AI 员工培训", level: 1 })).toBeTruthy();
    expect(screen.getByText("待学习任务")).toBeTruthy();
    expect(screen.getByText("知识库问答")).toBeTruthy();
    expect(screen.getByText("测验与错题")).toBeTruthy();
    expect(screen.getByText("引用来源")).toBeTruthy();
  });
});
