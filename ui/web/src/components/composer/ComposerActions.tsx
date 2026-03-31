import { EditIcon } from "../../pic/icon";

interface ComposerActionsProps {
  onNewChat: () => void;
}

export default function ComposerActions({ onNewChat }: ComposerActionsProps) {
  return (
    <div className="composer-actions">
      <button type="button" aria-label="新建对话" title="新建对话" onClick={onNewChat}>
        <span className="composer-action-icon" aria-hidden="true">
          <EditIcon />
        </span>
      </button>
    </div>
  );
}
