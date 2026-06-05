import type { LucideIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";

interface FoundationPlaceholderProps {
  title: string;
  label: string;
  description: string;
  permission: string;
  boundary: string;
  nextMilestone: string;
  icon: LucideIcon;
}

export function FoundationPlaceholder({
  title,
  label,
  description,
  permission,
  boundary,
  nextMilestone,
  icon: Icon
}: FoundationPlaceholderProps) {
  return (
    <div className="mx-auto flex w-full max-w-7xl flex-col gap-4">
      <section className="rounded-lg border bg-background p-5 shadow-sm">
        <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
          <div className="min-w-0">
            <div className="inline-flex items-center gap-2 rounded-full border bg-muted/45 px-3 py-1 text-xs text-muted-foreground">
              <Icon className="size-3.5 text-primary" />
              {label}
            </div>
            <h1 className="mt-3 text-xl font-semibold">{title}</h1>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">{description}</p>
          </div>
          <Badge variant="outline" className="w-fit">
            M0 Skeleton
          </Badge>
        </div>
      </section>

      <section className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
        <div className="rounded-lg border bg-background p-5 shadow-sm">
          <div className="text-sm font-medium text-muted-foreground">Module Boundary</div>
          <p className="mt-2 text-sm leading-6">{boundary}</p>
        </div>
        <div className="rounded-lg border bg-background p-5 shadow-sm">
          <div className="text-sm font-medium text-muted-foreground">Control Plane</div>
          <div className="mt-3 grid gap-3 text-sm">
            <div className="flex items-center justify-between gap-3">
              <span className="text-muted-foreground">Permission</span>
              <code className="rounded border bg-muted px-2 py-1 text-xs">{permission}</code>
            </div>
            <div className="flex items-center justify-between gap-3">
              <span className="text-muted-foreground">Status</span>
              <span>Scaffolded</span>
            </div>
          </div>
        </div>
      </section>

      <section className="rounded-lg border bg-background p-5 shadow-sm">
        <div className="text-sm font-medium text-muted-foreground">Next Milestone</div>
        <p className="mt-2 text-sm leading-6">{nextMilestone}</p>
      </section>
    </div>
  );
}
