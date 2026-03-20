import { useState, useEffect, useCallback } from "react";
import { SlashCommandDto, SlashCommandsResponse } from "../types/gateway";

export function useSlashCommands(sessionId: string | null) {
  const [commands, setCommands] = useState<SlashCommandDto[]>([]);
  const [loading, setLoading] = useState(false);

  const fetchCommands = useCallback(async () => {
    if (!sessionId) return;
    setLoading(true);
    try {
      const baseUrl = (window as any).__GATEWAY_URL__ || import.meta.env.VITE_GATEWAY_URL;
      const res = await fetch(`${baseUrl}/api/v1/slash_commands`, {
        headers: { Authorization: `Bearer ${localStorage.getItem("openjax_token")}` },
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: SlashCommandsResponse = await res.json();
      setCommands(data.commands);
    } catch {
      // ignore fetch errors silently
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    fetchCommands();
  }, [fetchCommands]);

  const filterCommands = useCallback(
    (query: string): SlashCommandDto[] => {
      if (!query.startsWith("/")) return [];
      const prefix = query.slice(1).toLowerCase();
      if (!prefix) return commands;
      return commands.filter(
        (c) =>
          c.name.toLowerCase().startsWith(prefix) ||
          c.aliases.some((a) => a.toLowerCase().startsWith(prefix))
      );
    },
    [commands]
  );

  return { commands, filterCommands, loading, refetch: fetchCommands };
}
