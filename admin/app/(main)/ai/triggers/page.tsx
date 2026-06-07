"use client";

import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { listTriggerEvents } from "@/api/ai/trigger";
import { CapabilityRegistry } from "@/components/ai/capability-registry";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { TriggerEventResp } from "@/types/ai-trigger";

export default function AiTriggersPage() {
  const [events, setEvents] = useState<TriggerEventResp[]>([]);
  const [eventTotal, setEventTotal] = useState(0);
  const [eventsLoading, setEventsLoading] = useState(false);

  const loadEvents = useCallback(async () => {
    setEventsLoading(true);
    try {
      const result = await listTriggerEvents({ page: 1, size: 10 });
      setEvents(result.list);
      setEventTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Webhook 事件加载失败");
    } finally {
      setEventsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadEvents();
  }, [loadEvents]);

  return (
    <div className="grid gap-4">
      <CapabilityRegistry title="触发器" resource="triggers" permission="ai:trigger:list" />
      <PermissionGate permissions={["ai:trigger:list"]}>
        <section className="mx-auto grid w-full max-w-7xl gap-3 rounded-lg border bg-background p-4">
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div className="min-w-0">
              <h2 className="truncate text-base font-semibold">Webhook Events</h2>
              <div className="mt-1 text-xs text-muted-foreground">{eventTotal} 条事件</div>
            </div>
            <Button variant="outline" onClick={() => void loadEvents()} disabled={eventsLoading}>
              <RefreshCw className={eventsLoading ? "size-4 animate-spin" : "size-4"} />
              刷新
            </Button>
          </div>

          <div className="grid gap-2">
            {events.map((event) => (
              <article key={event.id} className="grid gap-3 rounded-md border p-3 text-sm">
                <div className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
                  <div className="min-w-0">
                    <div className="truncate font-medium">{event.triggerCode}</div>
                    <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                      <span>{event.createTime}</span>
                      <span>{event.idempotencyKey}</span>
                      {event.traceId ? <span>{`Trace #${event.traceId}`}</span> : null}
                      {eventName(event.eventPayload) ? <span>{eventName(event.eventPayload)}</span> : null}
                    </div>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Badge variant="secondary">{event.status}</Badge>
                    <Badge variant="outline">{event.sourceType}</Badge>
                    <Badge variant="outline">{event.targetKind}</Badge>
                  </div>
                </div>
                <pre className="max-h-32 overflow-auto rounded-md bg-muted p-3 text-xs leading-5">
                  {payloadPretty(event.eventPayload)}
                </pre>
                {event.errorMessage ? (
                  <div className="rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs text-destructive">
                    {event.errorMessage}
                  </div>
                ) : null}
              </article>
            ))}
            {!events.length ? (
              <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
                暂无 Webhook 事件
              </div>
            ) : null}
          </div>
        </section>
      </PermissionGate>
    </div>
  );
}

function payloadPretty(value: TriggerEventResp["eventPayload"]) {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function eventName(value: TriggerEventResp["eventPayload"]) {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    const event = value.event;
    return typeof event === "string" ? event : null;
  }
  return null;
}
