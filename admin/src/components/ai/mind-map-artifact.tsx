"use client";

import { Transformer } from "markmap-lib";
import { Markmap } from "markmap-view";
import { Maximize2 } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { MindMapContent, MindMapNode, StudioArtifactResp } from "@/types/ai";

const transformer = new Transformer();

export function MindMapArtifact({
  artifact,
  className
}: {
  artifact: StudioArtifactResp;
  className?: string;
}) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const markmapRef = useRef<Markmap | null>(null);
  const content = artifact.contentJson;
  const markdown = useMemo(() => mindMapContentToMarkdown(content), [content]);
  const summary = useMemo(() => {
    const root = content.nodes.find((node) => node.id === "root");
    return root?.summary || artifact.contentText || "";
  }, [artifact.contentText, content.nodes]);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) {
      return;
    }
    const { root } = transformer.transform(markdown);
    if (!markmapRef.current) {
      markmapRef.current = Markmap.create(svg, {
        autoFit: true,
        duration: 240,
        fitRatio: 0.96,
        initialExpandLevel: 3,
        maxWidth: 320,
        paddingX: 14
      });
    }
    markmapRef.current.setData(root);
    void markmapRef.current.fit();
  }, [markdown]);

  return (
    <section
      className={cn("grid gap-3 rounded-lg border bg-background p-4", className)}
      data-testid="mind-map-artifact"
    >
      <div className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <Maximize2 className="size-4 shrink-0 text-primary" />
            <h3 className="truncate text-sm font-medium">{artifact.title}</h3>
          </div>
          {summary ? <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">{summary}</p> : null}
        </div>
        <div className="flex flex-wrap gap-1.5">
          <Badge variant="outline">{content.nodes.length} 节点</Badge>
          <Badge variant="outline">{content.edges.length} 关系</Badge>
          {artifact.citations.length ? <Badge variant="secondary">{artifact.citations.length} 引用</Badge> : null}
        </div>
      </div>
      <div className="h-[560px] overflow-hidden rounded-md border bg-muted/10">
        <svg ref={svgRef} className="h-full w-full" role="img" aria-label={artifact.title} />
      </div>
      {artifact.citations.length ? (
        <div className="flex flex-wrap gap-2">
          {artifact.citations.slice(0, 12).map((citation) => (
            <Badge key={`${citation.documentId}-${citation.chunkId}`} variant="secondary">
              {citation.chunkId}
            </Badge>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function mindMapContentToMarkdown(content: MindMapContent) {
  const root = content.nodes.find((node) => node.id === "root") ?? content.nodes[0];
  if (!root) {
    return `# ${content.title || "思维导图"}`;
  }
  const childrenByParent = new Map<string, MindMapNode[]>();
  for (const edge of content.edges) {
    const child = content.nodes.find((node) => node.id === edge.target);
    if (!child) {
      continue;
    }
    const children = childrenByParent.get(edge.source) ?? [];
    children.push(child);
    childrenByParent.set(edge.source, children);
  }

  const lines = [`# ${markmapLabel(root)}`];
  appendMarkdownChildren(lines, root.id, childrenByParent, 1, new Set([root.id]));
  return lines.join("\n");
}

function appendMarkdownChildren(
  lines: string[],
  parentId: string,
  childrenByParent: Map<string, MindMapNode[]>,
  depth: number,
  seen: Set<string>
) {
  const children = childrenByParent.get(parentId) ?? [];
  for (const child of children) {
    if (seen.has(child.id)) {
      continue;
    }
    seen.add(child.id);
    lines.push(`${"  ".repeat(depth)}- ${markmapLabel(child)}`);
    appendMarkdownChildren(lines, child.id, childrenByParent, depth + 1, seen);
  }
}

function markmapLabel(node: MindMapNode) {
  const refs = node.citationRefs?.length ? ` [${node.citationRefs.join(", ")}]` : "";
  return `${node.label || node.summary || node.id}${refs}`;
}
