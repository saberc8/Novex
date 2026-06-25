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

  it("uses supplied Chinese map copy", () => {
    render(
      <ResearchMap
        graph={graph}
        selectedNodeId={null}
        onNodeSelect={() => {}}
        copy={{
          title: "研究图谱",
          description: "探索主题、证据、空白与实验之间的联系。",
          graphLabel: "研究关系图",
          nodeCount: (count) => `${count} 个节点`,
          noUsableNodes: "暂无可用图谱节点",
          noUsableNodesDescription: "覆盖受限时，来源警告和限制会显示在下方。",
          caveats: "限制",
          layers: {
            papers: "论文",
            people: "人物",
            projects: "项目",
            models: "模型",
            datasets: "数据集",
            benchmarks: "基准",
            questions: "问题",
            experiments: "实验"
          },
          nodeKinds: {
            topic: "主题",
            hotspot: "热点",
            paper: "论文",
            project: "项目",
            model: "模型",
            dataset: "数据集",
            benchmark: "基准",
            author: "作者",
            institution: "机构",
            open_question: "开放问题",
            experiment: "实验"
          },
          relationLabels: {
            supports: "支撑",
            implements: "实现",
            evaluates: "评测",
            extends: "扩展",
            reveals_gap: "揭示空白",
            leads_to: "导向",
            mentions: "提及"
          }
        }}
      />
    );

    expect(screen.getByText("研究图谱")).toBeTruthy();
    expect(screen.getByRole("button", { name: "论文" })).toBeTruthy();
    expect(screen.getByLabelText("研究关系图")).toBeTruthy();
    expect(screen.getByText("支撑")).toBeTruthy();
    expect(screen.queryByText("supports")).toBeNull();
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
