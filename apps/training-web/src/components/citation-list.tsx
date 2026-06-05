import { FileText } from "lucide-react";

export interface CitationItem {
  title: string;
  chunkId: string;
  excerpt: string;
  score: string;
}

export function CitationList({ citations }: { citations: CitationItem[] }) {
  return (
    <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-slate-950">引用来源</h2>
        <span className="rounded-md bg-slate-100 px-2 py-1 text-xs text-slate-600">
          {citations.length} 条
        </span>
      </div>
      <div className="mt-3 space-y-3">
        {citations.map((citation) => (
          <article className="rounded-lg border border-slate-200 p-3" key={citation.chunkId}>
            <div className="flex items-start gap-2">
              <FileText aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-teal-700" />
              <div className="min-w-0">
                <div className="truncate text-sm font-medium text-slate-900">{citation.title}</div>
                <div className="mt-1 text-xs text-slate-500">
                  {citation.chunkId} · {citation.score}
                </div>
              </div>
            </div>
            <p className="mt-3 text-sm leading-6 text-slate-600">{citation.excerpt}</p>
          </article>
        ))}
      </div>
    </section>
  );
}
