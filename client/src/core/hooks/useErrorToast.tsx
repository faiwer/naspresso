import { WarningFilled } from "@ant-design/icons";
import { App } from "antd";
import type { ReactNode } from "react";

import { useAsyncEffect } from "./useAsyncEffect";

type UseErrorToastOptions = {
  /** Heading shown above the toast. Default: a generic apology. */
  title?: string;
  /**
   * Body text. Defaults to `error`'s message. Pass a domain-aware sentence
   * when the raw error isn't user-friendly, and keep passing `error` itself:
   * a repeated failure often maps to the same sentence, and an unchanged
   * value doesn't re-fire.
   */
  description?: string;
  /** Replaces the notification's status icon. */
  icon?: ReactNode;
};

const DEFAULT_TITLE = "Oops. Something went wrong";

/**
 * Pops an error toast when `error` becomes a value. The same error doesn't
 * re-fire; a fresh one, or a clear then a new set, does.
 *
 * `Error` instances render as their message, other values are stringified,
 * and `null` / `undefined` do nothing.
 */
export const useErrorToast = (
  error: unknown,
  options: UseErrorToastOptions = {},
): void => {
  const { notification } = App.useApp();

  useAsyncEffect(() => {
    const message =
      error instanceof Error ? error.message : error ? String(error) : null;
    if (!message) return;

    const { title = DEFAULT_TITLE, description, icon } = options;
    notification.error({
      title,
      description: description ?? message,
      placement: "bottomRight",
      icon: icon ?? <WarningFilled />,
    });
  }, [error]);
};
