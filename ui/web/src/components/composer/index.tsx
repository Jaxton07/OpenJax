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
  contextUsage?: ContextUsageState | null;
  policyLevel?: "allow" | "ask" | "deny";
  onPolicyLevelChange?: (level: "allow" | "ask" | "deny") => void;
}

export default function Composer({
  disabled,
  baseUrl,
  accessToken,
  sessionId,
  onSend,
  onNewChat,
  contextUsage,
  policyLevel,
  onPolicyLevelChange,
}: ComposerProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { commands, filterCommands } = useSlashCommands(baseUrl, accessToken);

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
    const trimmed = input.trim();
    if (!trimmed) return;

    // 检测是否是斜杠命令（支持带尾随空格的输入，如从下拉选中后的 "/explain "）
    const slashMatch = trimmed.match(/^\/([\w]+)$/);
    if (slashMatch) {
      const cmdName = slashMatch[1].toLowerCase();
      // 优先在完整命令列表中查找（不依赖 slashMatches 状态，后者可能因输入变化而过期）
      const matched = commands.find(
        (c) => c.name === cmdName || c.aliases.includes(cmdName)
      );

      if (matched) {
        if (matched.kind === "session_action") {
          // clear / compact：调用 /slash API，需要 session_id
          if (!sessionId) {
            setInput("");
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
          setInput("");
          const el = textareaRef.current;
          if (el) el.style.height = "";
          setShowSlashDropdown(false);
          return;
        }

        // builtin（如 help）和 skill 类型：作为普通 turn 提交，agent 响应
      }
    }

    // 普通消息提交
    const content = trimmed;
    setInput("");
    const el = textareaRef.current;
    if (el) el.style.height = "";
    setShowSlashDropdown(false);
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
        policyLevel={policyLevel}
        onPolicyLevelChange={onPolicyLevelChange}
      />
    </div>
  );
}
