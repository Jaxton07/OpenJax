import { SlashCommandDto } from "../../types/gateway";

interface SlashDropdownProps {
  visible: boolean;
  commands: SlashCommandDto[];
  selectedIndex: number;
  onSelect: (command: SlashCommandDto) => void;
}

export default function SlashDropdown({
  visible,
  commands,
  selectedIndex,
  onSelect,
}: SlashDropdownProps) {
  if (!visible || commands.length === 0) return null;

  return (
    <div className="slash-dropdown">
      {commands.map((cmd, i) => (
        <div
          key={cmd.name}
          className={`slash-dropdown-item ${i === selectedIndex ? "selected" : ""}`}
          onMouseDown={(e) => {
            e.preventDefault();
            onSelect(cmd);
          }}
        >
          <span className="slash-dropdown-name">/{cmd.name}</span>
          <span className="slash-dropdown-desc">{cmd.description}</span>
        </div>
      ))}
    </div>
  );
}
