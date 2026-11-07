import { LockFilled, UnlockFilled } from "@ant-design/icons";
import { Alert, App, Button } from "antd";
import { useState } from "react";

import {
  lockNPED,
  MAX_NPE_COOKIE_DURATION_M,
  NPE_PASSWORD_LENGTH,
  unlockNPED,
} from "../../../api/crypto";
import { formatLockRemaining, useCountdown } from "../../../core/helpers/date";
import { nullthrows } from "../../../core/helpers/nullthrows";
import { useAsync } from "../../../core/hooks/useAsync";
import { useErrorToast } from "../../../core/hooks/useErrorToast";
import { ALERT_STYLES } from "./alertStyles";
import { getFailureMessage, PasswordField } from "./form";

import styles from "./EncryptionPanel.module.css";

type UnlockedPanelProps = {
  vaultPath: string;
  deadline: number;
  onLocked: () => void;
  onPostponed: (expiredAt: number) => void;
};

export const UnlockedPanel = ({
  vaultPath,
  deadline,
  onLocked,
  onPostponed,
}: UnlockedPanelProps) => {
  const [showPostponeForm, setShowPostponeForm] = useState(false);
  const remaining = useCountdown(deadline);

  const [lock, isLockLoading, lockError] = useAsync(async () => {
    await lockNPED(vaultPath);
    onLocked();
  });

  useErrorToast(lockError, {
    title: "Couldn't lock this folder",
    icon: <LockFilled />,
  });

  return (
    <Alert
      className={styles.root}
      styles={ALERT_STYLES}
      type="success"
      showIcon
      icon={<UnlockFilled />}
      title={
        showPostponeForm ? (
          <>
            <p className={styles.text}>
              The folder will be locked in {formatLockRemaining(remaining)}.
              Click "Unlock" to postpone for another {MAX_NPE_COOKIE_DURATION_M}{" "}
              minutes.
            </p>
            <PostponeForm
              closeForm={() => setShowPostponeForm(false)}
              vaultPath={vaultPath}
              onPostponed={onPostponed}
            />
          </>
        ) : (
          <>
            <p className={styles.text}>
              The folder is encrypted and unlocked. It'll be locked in{" "}
              {formatLockRemaining(remaining)}.
            </p>
            {/* Lock and postpone aren't on the server yet. */}
            <div className={styles.controls}>
              <Button
                onClick={lock}
                loading={isLockLoading}
                disabled={isLockLoading}
              >
                Lock
              </Button>
              <Button onClick={() => setShowPostponeForm(true)}>
                Postpone
              </Button>
            </div>
          </>
        )
      }
    />
  );
};

const PostponeForm = ({
  closeForm,
  vaultPath,
  onPostponed,
}: {
  closeForm: () => void;
  vaultPath: string;
  onPostponed: (expiredAt: number) => void;
}) => {
  const { notification } = App.useApp();
  const [password, setPassword] = useState("");

  const [postpone, isPostponeLoading, postponeError] = useAsync(async () => {
    const result = await unlockNPED(vaultPath, password);
    if (result.unlocked) {
      onPostponed(nullthrows(result.expiredAt));
      closeForm();
    } else {
      notification.error({
        title: "Couldn't unlock this folder",
        description: getFailureMessage(result.error),
        placement: "bottomRight",
        icon: <LockFilled />,
      });
    }
  });

  useErrorToast(postponeError, { icon: <LockFilled /> });

  return (
    <form
      className={styles.controls}
      onSubmit={(event) => {
        event.preventDefault();
        void postpone();
      }}
    >
      <PasswordField password={password} setPassword={setPassword} />
      <Button
        type="primary"
        htmlType="submit"
        loading={isPostponeLoading}
        disabled={password.length !== NPE_PASSWORD_LENGTH}
      >
        Unlock
      </Button>
      <Button onClick={closeForm}>Cancel</Button>
    </form>
  );
};
