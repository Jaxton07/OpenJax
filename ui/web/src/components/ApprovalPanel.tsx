import type { PendingApproval } from "../types/chat";

interface ApprovalPanelProps {
  approvals: PendingApproval[];
  onResolve: (approval: PendingApproval, approved: boolean) => Promise<void> | void;
}

export default function ApprovalPanel({ approvals, onResolve }: ApprovalPanelProps) {
  if (approvals.length === 0) {
    return null;
  }

  return (
    <div className="approval-panel">
      <h3>待审批操作</h3>
      {approvals.map((approval) => (
        <div key={approval.approvalId} className="approval-item">
          <div className="approval-content">
            <strong>{approval.toolName || "tool"}</strong>
            <p>{approval.target || ""}</p>
            <small>{approval.reason || ""}</small>
          </div>
          <div className="approval-actions">
            <button onClick={() => void onResolve(approval, false)}>拒绝</button>
            <button className="primary" onClick={() => void onResolve(approval, true)}>
              允许
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
