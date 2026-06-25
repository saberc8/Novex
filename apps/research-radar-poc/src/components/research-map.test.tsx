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
      id: "author:ada",
      kind: "author",
      title: "Ada Lovelace",
      summary: "paper author",
      importance: 0.55,
      sourceItemIds: ["arxiv:1"],
      tags: ["people"]
    },
    {
      id: "institution:openai",
      kind: "institution",
      title: "OpenAI",
      summary: "research institution",
      importance: 0.5,
      sourceItemIds: ["arxiv:1"],
      tags: ["people"]
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
    },
    {
      id: "source:arxiv:1->author:ada:mentions",
      from: "source:arxiv:1",
      to: "author:ada",
      relation: "mentions",
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

  it("exposes useful hover metadata and selected state for nodes", () => {
    render(
      <ResearchMap graph={graph} selectedNodeId="source:arxiv:1" onNodeSelect={() => {}} />
    );

    const paperNode = screen.getByRole("button", { name: /Workflow Planning for AI Agents/ });

    expect(paperNode.getAttribute("title")).toBe("Workflow Planning for AI Agents - paper - paper summary");
    expect(paperNode.getAttribute("aria-pressed")).toBe("true");
    expect(paperNode.className).toContain("ring-2");
  });

  it("hides paper nodes when the Papers layer is disabled", () => {
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "Papers" }));

    expect(screen.queryByRole("button", { name: /Workflow Planning for AI Agents/ })).toBeNull();
    expect(screen.getByRole("button", { name: /agent workflow/ })).toBeTruthy();
  });

  it("hides author and institution nodes when the People layer is disabled", () => {
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "People" }));

    expect(screen.queryByRole("button", { name: /Ada Lovelace/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /OpenAI/ })).toBeNull();
    expect(screen.getByRole("button", { name: /agent workflow/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /planning/ })).toBeTruthy();
  });

  it("shows an empty state instead of a blank map when there are no usable nodes", () => {
    render(
      <ResearchMap
        graph={{ topic: "agent workflow", nodes: [], edges: [], caveats: ["leaderboards unavailable"] }}
        selectedNodeId={null}
        onNodeSelect={() => {}}
      />
    );

    expect(screen.getByText("No usable graph nodes")).toBeTruthy();
    expect(screen.getByText("leaderboards unavailable")).toBeTruthy();
  });
});
