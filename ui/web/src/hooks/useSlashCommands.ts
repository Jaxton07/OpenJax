import { useState, useEffect, useCallback } from "react";
import { SlashCommandDto, SlashCommandsResponse } from "../types/gateway";

function isVisibleInWebComposer(command: SlashCommandDto): boolean {
  return command.name !== "policy";
}

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
      const visibleCommands = commands.filter(isVisibleInWebComposer);
      if (!prefix) return visibleCommands;
      return visibleCommands.filter(
        (c) =>
          c.name.toLowerCase().startsWith(prefix) ||
          c.aliases.some((a) => a.toLowerCase().startsWith(prefix))
      );
    },
    [commands]
  );

  const findCommand = useCallback(
    (query: string): SlashCommandDto | undefined => {
      const normalized = query.trim().replace(/\s+$/, "").replace(/^\//, "").toLowerCase();
      if (!normalized) {
        return undefined;
      }
      return commands.find(
        (command) =>
          command.name.toLowerCase() === normalized ||
          command.aliases.some((alias) => alias.toLowerCase() === normalized)
      );
    },
    [commands]
  );

  return { commands, filterCommands, findCommand, loading, refetch: fetchCommands };
}
