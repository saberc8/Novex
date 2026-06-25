import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ResearchMap } from "./research-map";
import type { ResearchGraph } from "@/types/research";

const graph: ResearchGraph = {
  topic: "agent workflow",
  caveats: ["partial coverage"],
  nodes: [
    {
      id: "topic:agent-workflow",
      kind: "topic",
      title: "agent workflow",
      summary: "central topic",
      importance: 1,
      sourceItemIds: [],
      tags: []
    },
    {
      id: "hotspot:planning",
      kind: "hotspot",
      title: "planning",
      summary: "recurring hotspot",
      importance: 0.8,
      sourceItemIds: [],
      tags: ["planning"]
    },
    {
      id: "source:arxiv:1",
      kind: "paper",
      title: "Workflow Planning for AI Agents",
      summary: "paper summary",
      importance: 0.6,
      sourceItemIds: ["arxiv:1"],
      tags: ["planning"]
    },
    {
      id: "experiment:compare-runtimes",
      kind: "experiment",
      title: "Compare workflow runtimes",
      summary: "candidate experiment",
      importance: 0.7,
      sourceItemIds: [],
      tags: []
    }
  ],
  edges: [
    {
      id: "topic:agent-workflow->hotspot:planning:mentions",
      from: "topic:agent-workflow",
      to: "hotspot:planning",
      relation: "mentions",
      evidenceItemIds: []
    },
    {
      id: "hotspot:planning->source:arxiv:1:supports",
      from: "hotspot:planning",
      to: "source:arxiv:1",
      relation: "supports",
      evidenceItemIds: ["arxiv:1"]
    }
  ]
};

describe("ResearchMap", () => {
  it("renders a research map with nodes and relations", () => {
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={() => {}} />);

    expect(screen.getByText("Research Map")).toBeTruthy();
    expect(screen.getByRole("button", { name: /agent workflow/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /planning/ })).toBeTruthy();
    expect(screen.getByText("supports")).toBeTruthy();
  });

  it("selects a node when clicked", () => {
    const onNodeSelect = vi.fn();
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={onNodeSelect} />);

    fireEvent.click(screen.getByRole("button", { name: /Workflow Planning for AI Agents/ }));

    expect(onNodeSelect).toHaveBeenCalledWith("source:arxiv:1");
  });

  it("hides paper nodes when the Papers layer is disabled", () => {
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "Papers" }));

    expect(screen.queryByRole("button", { name: /Workflow Planning for AI Agents/ })).toBeNull();
    expect(screen.getByRole("button", { name: /agent workflow/ })).toBeTruthy();
  });
});
