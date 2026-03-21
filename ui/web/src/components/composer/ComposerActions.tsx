import { PlusIcon } from "../../pic/icon";

interface ComposerActionsProps {
  onNewChat: () => void;
}

export default function ComposerActions({ onNewChat }: ComposerActionsProps) {
  return (
    <div className="composer-actions">
      <button type="button" onClick={onNewChat}>
        <span className="composer-action-icon" aria-hidden="true">
          <PlusIcon />
        </span>
        <span>新建对话</span>
      </button>
    </div>
  );
}
