import { getEncryptedDirPath } from "../../../api/storage";
import type { Encryption } from "../../../api/storage.io";
import { nullthrows } from "../../../core/helpers/nullthrows";
import { LockedPanel } from "./LockedPanel";
import { UnlockedPanel } from "./UnlockedPanel";

type EncryptionPanelProps = {
  /** The listed directory. Remount the panel (`key`) to point it at another. */
  segments: string[];
  encryption: Encryption;
  /** Unix milliseconds when the folder locks again. `null` while locked. */
  lockAt: number | null;
  /** Refetch the listing after the directory opens. */
  onUnlocked: (unlockAt: number) => void;
  /** Refetch the listing after the directory locks. */
  onLocked: () => void;
};

/**
 * Banner for a directory inside an `.nped` folder. Locked asks for the password;
 * unlocked counts down until the cookie lapses. Renders nothing otherwise.
 */
export const EncryptionPanel = ({
  segments,
  encryption,
  lockAt,
  onUnlocked,
  onLocked,
}: EncryptionPanelProps) => {
  const path = nullthrows(getEncryptedDirPath(segments));

  if (encryption === "unlocked") {
    return (
      <UnlockedPanel
        vaultPath={path}
        deadline={nullthrows(lockAt)}
        onLocked={onLocked}
      />
    );
  }

  if (encryption === "locked") {
    return <LockedPanel vaultPath={path} onUnlocked={onUnlocked} />;
  }

  return null;
};
