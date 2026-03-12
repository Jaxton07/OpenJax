import { useState } from "react";

interface LoginPageProps {
  initialBaseUrl: string;
  onLogin: (baseUrl: string, ownerKey: string) => Promise<boolean>;
  errorMessage?: string | null;
}

export default function LoginPage(props: LoginPageProps) {
  const [baseUrl, setBaseUrl] = useState(props.initialBaseUrl);
  const [ownerKey, setOwnerKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const formReady = baseUrl.trim().length > 0 && ownerKey.trim().length > 0;

  const submit = async () => {
    if (submitting) {
      return;
    }
    setSubmitting(true);
    setStatus(null);
    try {
      const ok = await props.onLogin(baseUrl, ownerKey);
      if (!ok) {
        setStatus("连接失败，请检查地址和 Owner Key。");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="login-page">
      <div className="login-card">
        <div className="login-brand-row">
          <div className="login-badge">OpenJax</div>
          <div className="login-sub-badge">Owner Key</div>
        </div>
        <h1>登录控制台</h1>
        <p>请填写网关地址与 Owner Key。</p>

        <div className="login-tip">
          <strong>快速步骤</strong>
          <span>1. 启动 `openjax-gateway`</span>
          <span>2. 复制终端输出的 `ojx_...` owner key 并输入</span>
        </div>

        <label>
          Gateway Base URL
          <input
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
            placeholder="http://127.0.0.1:8765"
          />
        </label>

        <label>
          Owner Key
          <div className="login-key-row">
            <input
              type={showKey ? "text" : "password"}
              value={ownerKey}
              onChange={(event) => setOwnerKey(event.target.value)}
              placeholder="ojx_xxxxxxxxxxxxxxxxx"
              onKeyDown={(event) => {
                if (event.key === "Enter" && formReady) {
                  void submit();
                }
              }}
            />
            <button
              type="button"
              className="key-visibility-btn"
              onClick={() => setShowKey((prev) => !prev)}
              aria-label={showKey ? "隐藏 Owner Key" : "显示 Owner Key"}
              title={showKey ? "隐藏 Owner Key" : "显示 Owner Key"}
            >
              {showKey ? (
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M3.53 2.47a.75.75 0 10-1.06 1.06l2.04 2.04A12.84 12.84 0 001.6 12a12.77 12.77 0 004.95 5.1 12.96 12.96 0 0011.88.87l2.04 2.03a.75.75 0 101.06-1.06L3.53 2.47zm11.89 11.89a4.5 4.5 0 01-5.78-5.78l1.18 1.18a3 3 0 003.42 3.42l1.18 1.18zM12 6.75c2.26 0 4.2 1.03 5.7 2.26 1.33 1.08 2.28 2.27 2.8 2.99a14.29 14.29 0 01-2.3 2.5.75.75 0 10.92 1.18 15.84 15.84 0 002.83-3.18.75.75 0 000-.82c-.58-.87-1.73-2.33-3.3-3.62C17.08 6.8 14.8 5.25 12 5.25c-.96 0-1.89.18-2.77.5a.75.75 0 00.51 1.42A6.9 6.9 0 0112 6.75zm-4.18 1.2a.75.75 0 10-.97 1.14A4.48 4.48 0 009 15.2a.75.75 0 001.05-1.07A2.98 2.98 0 017.9 10a2.94 2.94 0 00-.08.7c0 .22.02.44.05.65a.75.75 0 101.48-.22A1.5 1.5 0 019 10.7c0-.23.05-.45.14-.66a.75.75 0 00-1.32-.67z" />
                </svg>
              ) : (
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M12 5.25c2.8 0 5.08 1.55 6.64 2.84a15.8 15.8 0 013.31 3.62.75.75 0 010 .82c-.58.87-1.73 2.33-3.3 3.62-1.57 1.29-3.85 2.84-6.65 2.84-2.8 0-5.08-1.55-6.65-2.84A15.84 15.84 0 012.05 12a.75.75 0 010-.82c.58-.87 1.73-2.33 3.3-3.62C6.92 6.8 9.2 5.25 12 5.25zm0 1.5c-2.26 0-4.2 1.03-5.7 2.26A14.33 14.33 0 003.95 12c.5.72 1.45 1.9 2.79 2.99 1.5 1.23 3.44 2.26 5.7 2.26 2.25 0 4.2-1.03 5.69-2.26A14.35 14.35 0 0020.05 12c-.5-.72-1.46-1.91-2.8-2.99-1.5-1.23-3.43-2.26-5.69-2.26zm0 2.25a3.75 3.75 0 110 7.5 3.75 3.75 0 010-7.5zm0 1.5a2.25 2.25 0 100 4.5 2.25 2.25 0 000-4.5z" />
                </svg>
              )}
            </button>
          </div>
        </label>

        <button
          className="primary login-submit-btn"
          onClick={() => void submit()}
          disabled={submitting || !formReady}
        >
          {submitting ? "进入中..." : "进入"}
        </button>

        {status ? <div className="status-tip">{status}</div> : null}
        {props.errorMessage ? <div className="status-tip error-tip">{props.errorMessage}</div> : null}
      </div>
    </div>
  );
}
