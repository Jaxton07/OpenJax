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
    // ignore body parse errors
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
        return "认证失败，请检查 API Key。";
      case "FORBIDDEN":
        return "权限不足，请更换 API Key。";
      case "RATE_LIMITED":
        return "请求过于频繁，请稍后重试。";
      case "UPSTREAM_UNAVAILABLE":
      case "TIMEOUT":
        return "网关暂不可用，可稍后重试。";
      case "NOT_IMPLEMENTED":
        return "该能力尚未实现。";
      default:
        return gateway.message;
    }
  }

  if (error instanceof Error) {
    return error.message;
  }

  return "发生未知错误";
}
