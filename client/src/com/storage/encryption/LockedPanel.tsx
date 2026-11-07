import { LockFilled, UnlockFilled } from "@ant-design/icons";
import { Alert, App, Button } from "antd";
import { useState } from "react";

import { unlockNPED, NPE_PASSWORD_LENGTH } from "../../../api/crypto";
import { nullthrows } from "../../../core/helpers/nullthrows";
import { useAsync } from "../../../core/hooks/useAsync";
import { useErrorToast } from "../../../core/hooks/useErrorToast";
import { ALERT_STYLES } from "./alertStyles";
import { getFailureMessage, PasswordField } from "./form";

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
            <PasswordField password={password} setPassword={setPassword} />
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
