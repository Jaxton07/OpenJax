import { useLayoutEffect, useRef, useState } from "react";
import "./composer.css";
import ComposerActions from "./ComposerActions";
import ComposerInput from "./ComposerInput";
import SlashDropdown from "./SlashDropdown";
import { useSlashCommands } from "../../hooks/useSlashCommands";
import type { ContextUsageState } from "../../types/chat";
import type { SlashCommandDto } from "../../types/gateway";

interface ComposerProps {
  disabled?: boolean;
  baseUrl: string;
  accessToken: string;
  sessionId?: string | null;
  onSend: (content: string) => Promise<void> | void;
  onNewChat: () => void;
  onClear?: () => Promise<void> | void;
  contextUsage?: ContextUsageState | null;
  policyLevel?: "allow" | "ask" | "deny";
  onPolicyLevelChange?: (level: "allow" | "ask" | "deny") => void;
  isBusyTurn?: boolean;
  isStreaming?: boolean;
  onBlockedSendAttempt?: () => void;
  onStop?: () => void;
}

export default function Composer({
  disabled,
  baseUrl,
  accessToken,
  sessionId,
  onSend,
  onNewChat,
  onClear,
  contextUsage,
  policyLevel,
  onPolicyLevelChange,
  isBusyTurn,
  isStreaming,
  onBlockedSendAttempt,
  onStop
}: ComposerProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { commands, filterCommands, findCommand } = useSlashCommands(baseUrl, accessToken);

  const [showSlashDropdown, setShowSlashDropdown] = useState(false);
  const [slashSelectedIndex, setSlashSelectedIndex] = useState(0);
  const [slashMatches, setSlashMatches] = useState<SlashCommandDto[]>([]);

  const resizeTextarea = () => {
    const el = textareaRef.current;
    if (!el) {
      return;
    }
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
  };

  useLayoutEffect(() => {
    resizeTextarea();
  }, [input]);

  // Detect slash command in input and update dropdown
  useLayoutEffect(() => {
    const slashMatch = input.match(/^\/(\w*)$/);
    if (slashMatch) {
      const query = slashMatch[0];
      const matches = filterCommands(query);
      setSlashMatches(matches);
      setShowSlashDropdown(matches.length > 0);
      setSlashSelectedIndex(0);
    } else {
      setShowSlashDropdown(false);
    }
  }, [input, filterCommands]);

  const resetComposer = () => {
    setInput("");
    const el = textareaRef.current;
    if (el) el.style.height = "";
    setShowSlashDropdown(false);
  };

  const submitResolvedCommand = async (command: SlashCommandDto) => {
    const cmdName = command.name.toLowerCase();

    if (cmdName === "policy") {
      resetComposer();
      return;
    }

    if (cmdName === "clear") {
      resetComposer();
      await onClear?.();
      return;
    }

    if (command.kind === "session_action") {
      if (!sessionId) {
        resetComposer();
        return;
      }
      try {
        await fetch(`${baseUrl}/api/v1/sessions/${sessionId}/slash`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${accessToken}`,
          },
          body: JSON.stringify({ command: cmdName }),
        });
      } catch {
        // 网络失败时静默忽略，避免 UI 卡住
      }
      resetComposer();
      return;
    }

    if (command.kind === "local_picker") {
      resetComposer();
      return;
    }

    resetComposer();
    try {
      await onSend(`/${cmdName}`);
    } catch {
      // 忽略
    }
  };

  const submit = async () => {
    if (isBusyTurn) {
      onBlockedSendAttempt?.();
      return;
    }
    const trimmed = input.trim();
    if (!trimmed) return;

    const slashMatch = trimmed.match(/^\/([\w]+)$/);
    if (slashMatch) {
      const matched = findCommand(trimmed);
      if (matched) {
        await submitResolvedCommand(matched);
        return;
      }
    }

    const content = trimmed;
    resetComposer();
    try {
      await onSend(content);
    } catch {
      // 忽略
    }
  };

  const handleSlashClose = () => {
    setShowSlashDropdown(false);
  };

  const handleSlashSelect = (cmd: SlashCommandDto) => {
    setInput(`/${cmd.name} `);
    setShowSlashDropdown(false);
    textareaRef.current?.focus();
  };

  const handleSlashSubmit = async () => {
    if (isBusyTurn) {
      onBlockedSendAttempt?.();
      return;
    }
    const matched = slashMatches[effectiveSlashIndex];
    if (!matched) {
      await submit();
      return;
    }
    await submitResolvedCommand(matched);
  };

  // Clamp selected index to valid range when dropdown is visible
  const effectiveSlashIndex =
    slashSelectedIndex < 0
      ? slashMatches.length - 1
      : slashSelectedIndex >= slashMatches.length
        ? 0
        : slashSelectedIndex;

  return (
    <div className="composer-wrap">
      <ComposerActions onNewChat={onNewChat} />
      <SlashDropdown
        visible={showSlashDropdown}
        commands={slashMatches}
        selectedIndex={effectiveSlashIndex}
        onSelect={handleSlashSelect}
      />
      <ComposerInput
        value={input}
        disabled={disabled}
        textareaRef={textareaRef}
        onChange={setInput}
        onSubmit={() => void submit()}
        contextUsage={contextUsage}
        showSlashDropdown={showSlashDropdown}
        slashSelectedIndex={effectiveSlashIndex}
        onSlashIndexChange={(i) => setSlashSelectedIndex(i)}
        onSlashClose={handleSlashClose}
        onSlashSubmit={() => void handleSlashSubmit()}
        policyLevel={policyLevel}
        onPolicyLevelChange={onPolicyLevelChange}
        isBusyTurn={isBusyTurn}
        isStreaming={isStreaming}
        onStop={onStop}
      />
    </div>
  );
}
