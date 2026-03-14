import type { GatewayError, GatewayErrorEnvelope } from "../types/gateway";

export async function parseGatewayError(response: Response): Promise<GatewayError> {
  let message = `HTTP ${response.status}`;
  let code = "INTERNAL";
  let retryable = false;

  try {
    const body = (await response.json()) as GatewayErrorEnvelope;
    message = body.error?.message ?? message;
    code = body.error?.code ?? code;
    retryable = body.error?.retryable ?? retryable;
  } catch {
    // ignore parse failure
  }

  const error = new Error(message) as GatewayError;
  error.status = response.status;
  error.code = code;
  error.retryable = retryable;
  return error;
}

export function humanizeError(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error) {
    const gateway = error as GatewayError;
    switch (gateway.code) {
      case "UNAUTHENTICATED":
        return "认证失败，请检查 Owner Key 或重新登录。";
      case "NOT_FOUND":
        return "会话不存在，建议重新创建会话。";
      case "TIMEOUT":
      case "UPSTREAM_UNAVAILABLE":
        return "网关暂不可用，请稍后重试。";
      case "INVALID_ARGUMENT":
        return "请求参数无效，请刷新后重试。";
      default:
        return gateway.message;
    }
  }

  if (error instanceof Error) {
    return error.message;
  }

  return "发生未知错误";
}

export function isAuthError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const gateway = error as Partial<GatewayError>;
  return gateway.code === "UNAUTHENTICATED" || gateway.status === 401 || gateway.status === 403;
}
