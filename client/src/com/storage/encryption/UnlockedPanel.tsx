import { LockFilled, UnlockFilled } from "@ant-design/icons";
import { Alert, Button } from "antd";

import { lockNPED } from "../../../api/crypto";
import { formatLockRemaining, useCountdown } from "../../../core/helpers/date";
import { useAsync } from "../../../core/hooks/useAsync";
import { useErrorToast } from "../../../core/hooks/useErrorToast";
import { ALERT_STYLES } from "./alertStyles";

import styles from "./EncryptionPanel.module.css";

type UnlockedPanelProps = {
  vaultPath: string;
  deadline: number;
  onLocked: () => void;
};

export const UnlockedPanel = ({
  vaultPath,
  deadline,
  onLocked,
}: UnlockedPanelProps) => {
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
          </div>
        </>
      }
    />
  );
};
