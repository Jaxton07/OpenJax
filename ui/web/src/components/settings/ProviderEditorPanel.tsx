import { useEffect, useRef } from "react";
import ProviderForm, { type ProviderFormValue } from "./ProviderForm";

const PROVIDER_CLOSE_ICON = new URL("../../pic/icon/close.svg", import.meta.url).href;

interface ProviderEditorPanelProps {
  closing: boolean;
  mode: "create" | "edit";
  initialValue: ProviderFormValue;
  submitting: boolean;
  scrollResetKey: string;
  onClose: () => void;
  onSubmit: (value: ProviderFormValue) => Promise<void>;
}

export default function ProviderEditorPanel(props: ProviderEditorPanelProps) {
  const wrapRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (props.closing) {
      return;
    }
    requestAnimationFrame(() => {
      if (wrapRef.current) {
        wrapRef.current.scrollTop = 0;
      }
    });
  }, [props.closing, props.scrollResetKey]);

  return (
    <div
      ref={wrapRef}
      className={props.closing ? "provider-form-wrap provider-form-wrap-closing" : "provider-form-wrap"}
      data-opened="true"
    >
      <button
        type="button"
        className="provider-form-close-btn"
        aria-label="关闭新增/编辑面板"
        onClick={props.onClose}
      >
        <img src={PROVIDER_CLOSE_ICON} alt="" />
      </button>
      <ProviderForm
        mode={props.mode}
        initialValue={props.initialValue}
        submitting={props.submitting}
        onSubmit={props.onSubmit}
        onCancelEdit={props.onClose}
      />
    </div>
  );
}
