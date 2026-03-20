import { CompactIcon, PlusIcon } from "../../pic/icon";

interface ComposerActionsProps {
  onNewChat: () => void;
  onCompact: () => void;
}

export default function ComposerActions({ onNewChat, onCompact }: ComposerActionsProps) {
  return (
    <div className="composer-actions">
      <button type="button" onClick={onNewChat}>
        <span className="composer-action-icon" aria-hidden="true">
          <PlusIcon />
        </span>
        <span>新建对话</span>
      </button>
      <button type="button" onClick={onCompact}>
        <span className="composer-action-icon" aria-hidden="true">
          <CompactIcon />
        </span>
        <span>压缩</span>
      </button>
    </div>
  );
}
