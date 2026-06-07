import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TrainingShell } from "./training-shell";

vi.mock("next/navigation", () => ({
  usePathname: () => "/quiz"
}));

describe("TrainingShell", () => {
  it("marks the active customer route in the sidebar", () => {
    render(
      <TrainingShell>
        <div>content</div>
      </TrainingShell>
    );

    const links = screen.getAllByRole("link");
    const learnLink = links.find((link) => link.getAttribute("href") === "/");
    const quizLink = links.find((link) => link.getAttribute("href") === "/quiz");

    expect(quizLink?.getAttribute("aria-current")).toBe("page");
    expect(learnLink?.getAttribute("aria-current")).toBeNull();
  });
});
