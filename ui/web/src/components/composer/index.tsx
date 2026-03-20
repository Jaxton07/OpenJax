import { useLayoutEffect, useRef, useState } from "react";
import "./composer.css";
import ComposerActions from "./ComposerActions";
import ComposerInput from "./ComposerInput";
import SlashDropdown from "./SlashDropdown";
import { useSlashCommands } from "../../hooks/useSlashCommands";
import type { SlashCommandDto } from "../../types/gateway";

interface ComposerProps {
  disabled?: boolean;
  sessionId?: string | null;
  onSend: (content: string) => Promise<void> | void;
  onNewChat: () => void;
  onCompact: () => void;
}

export default function Composer({ disabled, sessionId, onSend, onNewChat, onCompact }: ComposerProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { commands, filterCommands } = useSlashCommands(sessionId ?? null);

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

  const submit = async () => {
    // If a slash command session_action is selected, call the slash API
    const slashMatch = input.match(/^\/(\w+)$/);
    if (slashMatch) {
      const cmdName = slashMatch[1];
      const matched = slashMatches.find((c) => c.name === cmdName);
      if (matched?.kind === "session_action") {
        // Call /slash exec API then clear
        try {
          const baseUrl = (window as any).__GATEWAY_URL__ || import.meta.env.VITE_GATEWAY_URL;
          await fetch(`${baseUrl}/api/v1/slash`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${localStorage.getItem("openjax_token")}`,
            },
            body: JSON.stringify({ command: input }),
          });
        } catch {
          // ignore exec errors
        }
        setInput("");
        const el = textareaRef.current;
        if (el) el.style.height = "";
        return;
      }
    }

    const content = input.trim();
    if (!content) {
      return;
    }
    await onSend(content);
    setInput("");
    const el = textareaRef.current;
    if (el) {
      el.style.height = "";
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

  // Clamp selected index to valid range when dropdown is visible
  const effectiveSlashIndex =
    slashSelectedIndex < 0
      ? slashMatches.length - 1
      : slashSelectedIndex >= slashMatches.length
        ? 0
        : slashSelectedIndex;

  return (
    <div className="composer-wrap">
      <ComposerActions onNewChat={onNewChat} onCompact={onCompact} />
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
        showSlashDropdown={showSlashDropdown}
        slashSelectedIndex={effectiveSlashIndex}
        onSlashIndexChange={(i) => setSlashSelectedIndex(i)}
        onSlashClose={handleSlashClose}
      />
    </div>
  );
}
