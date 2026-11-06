import { LockFilled, UnlockFilled } from "@ant-design/icons";
import { Alert, App, Button, Input } from "antd";
import { useState } from "react";

import { unlockNPED, NPE_PASSWORD_LENGTH } from "../../../api/crypto";
import type { Unlock } from "../../../api/crypto.io";
import { nullthrows } from "../../../core/helpers/nullthrows";
import { useAsync } from "../../../core/hooks/useAsync";
import { useErrorToast } from "../../../core/hooks/useErrorToast";
import { ALERT_STYLES } from "./alertStyles";

import styles from "./EncryptionPanel.module.css";

type LockedPanelProps = {
  vaultPath: string;
  onUnlocked: (unlockAt: number) => void;
};

export const LockedPanel = ({ vaultPath, onUnlocked }: LockedPanelProps) => {
  const { notification } = App.useApp();
  const [password, setPassword] = useState("");
  const [onSubmit, isSubmitting, error] = useAsync(async () => {
    const result = await unlockNPED(vaultPath, password);

    if (!result.unlocked) {
      notification.error({
        title: "Couldn't unlock this folder",
        description: getFailureMessage(result.error),
        placement: "bottomRight",
        icon: <LockFilled />,
      });
      return;
    }

    notification.success({
      title: "Folder unlocked.",
      placement: "bottomRight",
      icon: <UnlockFilled />,
    });
    setPassword("");
    onUnlocked(nullthrows(result.expiredAt));
  });

  useErrorToast(error, {
    title: "Couldn't unlock this folder",
    icon: <LockFilled />,
  });

  return (
    <Alert
      className={styles.root}
      styles={ALERT_STYLES}
      type="warning"
      showIcon
      icon={<LockFilled />}
      title={
        <>
          <p className={styles.text}>
            The folder is encrypted. Enter your password to reveal its contents.
          </p>
          <form
            className={styles.controls}
            onSubmit={(event) => {
              event.preventDefault();
              void onSubmit();
            }}
          >
            <Input.Password
              className={styles.password}
              value={password}
              placeholder="Password"
              autoComplete="off"
              aria-label="Password"
              autoFocus
              onChange={(event) => setPassword(event.target.value)}
            />
            <Button
              type="primary"
              htmlType="submit"
              loading={isSubmitting}
              disabled={password.length !== NPE_PASSWORD_LENGTH}
            >
              Unlock
            </Button>
          </form>
        </>
      }
    />
  );
};

const getFailureMessage = (error: Unlock["error"] | null): string => {
  switch (error?.code) {
    case "WrongPassword":
      return "Wrong password.";
    case "NoEncryptedFile":
      return "This folder has no encrypted file to open.";
    default:
      return `Couldn't unlock this folder. ${error?.message}`;
  }
};
