import { streamRenderStore } from "../../lib/streamRenderStore";
import { saveAuth } from "../../lib/storage";
import type { ChatSession, ChatState } from "../../types/chat";
import type { GatewayClient } from "../../lib/gatewayClient";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";

interface ClearAuthStateParams {
  message: string;
  reconnectAbortRef: MutableRefObject<AbortController | null>;
  pollingAbortRef: MutableRefObject<AbortController | null>;
  sessionsRef: MutableRefObject<ChatSession[]>;
  setState: Dispatch<SetStateAction<ChatState>>;
}

export function clearAuthStateRuntime(params: ClearAuthStateParams): void {
  params.reconnectAbortRef.current?.abort();
  params.pollingAbortRef.current?.abort();
  for (const session of params.sessionsRef.current) {
    streamRenderStore.clear(session.id);
  }
  saveAuth({ authenticated: false, accessToken: "", sessionId: null, scope: null });
  params.setState((prev) => ({
    ...prev,
    auth: { authenticated: false, accessToken: "", sessionId: null, scope: null },
    sessions: [],
    activeSessionId: null,
    globalError: params.message
  }));
}

interface RefreshAccessTokenParams {
  client: GatewayClient;
  refreshPromiseRef: MutableRefObject<Promise<boolean> | null>;
  setState: Dispatch<SetStateAction<ChatState>>;
  clearAuthState: (message: string) => void;
}

export async function refreshAccessTokenRuntime(params: RefreshAccessTokenParams): Promise<boolean> {
  if (params.refreshPromiseRef.current) {
    return params.refreshPromiseRef.current;
  }

  const refreshing = (async () => {
    try {
      const refreshed = await params.client.refresh();
      saveAuth({
        authenticated: true,
        accessToken: "",
        sessionId: refreshed.session_id,
        scope: refreshed.scope
      });
      params.setState((prev) => ({
        ...prev,
        auth: {
          authenticated: true,
          accessToken: refreshed.access_token,
          sessionId: refreshed.session_id,
          scope: refreshed.scope
        }
      }));
      return true;
    } catch {
      params.clearAuthState("登录凭据已过期，请重新输入 Owner Key 登录。");
      return false;
    } finally {
      params.refreshPromiseRef.current = null;
    }
  })();

  params.refreshPromiseRef.current = refreshing;
  return refreshing;
}

export async function withAuthRetryRuntime<T>(
  action: () => Promise<T>,
  refreshAccessToken: () => Promise<boolean>,
  isAuthenticationError: (error: unknown) => boolean
): Promise<T> {
  try {
    return await action();
  } catch (error) {
    if (!isAuthenticationError(error)) {
      throw error;
    }
    const refreshed = await refreshAccessToken();
    if (!refreshed) {
      throw error;
    }
    return action();
  }
}
