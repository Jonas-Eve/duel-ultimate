import type { ErrorCode } from "../engine/errors";

export const ERROR_TEXT = {} as Record<ErrorCode, string>;
export const messageFor = (_code: string): string => "";
