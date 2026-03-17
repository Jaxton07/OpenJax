import { useEffect, useRef, useState } from "react";

interface ToastBannerProps {
  message: string;
  variant: "error" | "info";
  duration?: number;
  onDismiss: () => void;
}

export default function ToastBanner({ message, variant, duration = 4000, onDismiss }: ToastBannerProps) {
  const [progress, setProgress] = useState(100);
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    const start = Date.now();
    let rafId: number;

    const tick = () => {
      const elapsed = Date.now() - start;
      const pct = Math.max(0, (1 - elapsed / duration) * 100);
      setProgress(pct);
      if (pct > 0) {
        rafId = requestAnimationFrame(tick);
      } else {
        onDismissRef.current();
      }
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [duration]);

  return (
    <div
      className={`toast-banner toast-banner--${variant}`}
      onClick={() => onDismissRef.current()}
      role="alert"
      aria-live="polite"
    >
      <span className="toast-banner__message">{message}</span>
      <div className="toast-banner__progress-track">
        <div className="toast-banner__progress-bar" style={{ width: `${progress}%` }} />
      </div>
    </div>
  );
}
