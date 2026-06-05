import { ArrowUpRight, Clock, Target, Trophy } from "lucide-react";

interface Metric {
  label: string;
  value: string;
  detail: string;
  tone: "teal" | "amber" | "blue" | "rose";
}

const toneClass: Record<Metric["tone"], string> = {
  teal: "bg-teal-50 text-teal-800 ring-teal-100",
  amber: "bg-amber-50 text-amber-800 ring-amber-100",
  blue: "bg-blue-50 text-blue-800 ring-blue-100",
  rose: "bg-rose-50 text-rose-800 ring-rose-100"
};

const iconMap = [Target, Clock, Trophy, ArrowUpRight];

export function MetricStrip({ metrics }: { metrics: Metric[] }) {
  return (
    <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
      {metrics.map((metric, index) => {
        const Icon = iconMap[index] ?? Target;

        return (
          <div
            className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm"
            key={metric.label}
          >
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="text-xs font-medium text-slate-500">{metric.label}</div>
                <div className="mt-2 text-2xl font-semibold text-slate-950">{metric.value}</div>
              </div>
              <span className={`rounded-md p-2 ring-1 ${toneClass[metric.tone]}`}>
                <Icon aria-hidden="true" className="h-4 w-4" />
              </span>
            </div>
            <div className="mt-2 text-xs text-slate-500">{metric.detail}</div>
          </div>
        );
      })}
    </div>
  );
}
