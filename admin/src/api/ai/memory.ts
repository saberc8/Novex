import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type { MemoryCommand, MemoryQuery, MemoryResp } from "@/types/ai-memory";

const MEMORY_URL = "/ai/memories";

export function listMemories(query: MemoryQuery = {}) {
  return api.get<PageResult<MemoryResp>>(MEMORY_URL, { ...query });
}

export function upsertMemory(data: MemoryCommand) {
  return api.post<MemoryResp>(MEMORY_URL, data);
}

export function deleteMemory(id: number) {
  return api.delete<boolean>(`${MEMORY_URL}/${id}`);
}
