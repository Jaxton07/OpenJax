import { useState } from "react";
import { EyeOffIcon, EyeOpenIcon } from "../pic/icon";

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
              {showKey ? <EyeOffIcon aria-hidden="true" /> : <EyeOpenIcon aria-hidden="true" />}
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
