import React, { useState, useRef, useEffect } from "react";
import { UpDownIcon } from "../../pic/icon";
import "./composer.css";

interface PolicyLevelButtonProps {
  level: "allow" | "ask" | "deny";
  onChange: (level: "allow" | "ask" | "deny") => void;
}

const POLICY_OPTIONS: Array<{ level: "allow" | "ask" | "deny"; summary: string }> = [
  { level: "allow", summary: "Allow all tools without asking" },
  { level: "ask",   summary: "Ask before risky operations" },
  { level: "deny",  summary: "Deny all risky operations" },
];

export default function PolicyLevelButton({ level, onChange }: PolicyLevelButtonProps) {
  const [open, setOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div ref={wrapRef} className="policy-level-wrap">
      {open && (
        <div className="policy-level-popover">
          {POLICY_OPTIONS.map((opt) => (
            <div
              key={opt.level}
              className={`policy-level-option${opt.level === level ? " active" : ""}`}
              onMouseDown={(e) => {
                e.preventDefault();
                onChange(opt.level);
                setOpen(false);
              }}
            >
              <span className="policy-level-option-name">{opt.level}</span>
              <span className="policy-level-option-summary">{opt.summary}</span>
            </div>
          ))}
        </div>
      )}
      <button
        type="button"
        className="policy-level-btn"
        onClick={() => setOpen((v) => !v)}
        title="Change policy level"
      >
        {level}
        <UpDownIcon />
      </button>
    </div>
  );
}
