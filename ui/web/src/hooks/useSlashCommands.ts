import { useState, useEffect, useCallback } from "react";
import { SlashCommandDto, SlashCommandsResponse } from "../types/gateway";

export function useSlashCommands(baseUrl: string, accessToken: string) {
  const [commands, setCommands] = useState<SlashCommandDto[]>([]);
  const [loading, setLoading] = useState(false);

  const fetchCommands = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch(`${baseUrl}/api/v1/slash_commands`, {
        headers: { Authorization: `Bearer ${accessToken}` },
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: SlashCommandsResponse = await res.json();
      setCommands(data.commands);
    } catch {
      // ignore fetch errors silently
    } finally {
      setLoading(false);
    }
  }, [accessToken, baseUrl]);

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
