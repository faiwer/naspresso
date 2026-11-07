import { Input } from "antd";

import type { Unlock } from "../../../api/crypto.io";

import styles from "./EncryptionPanel.module.css";
export const getFailureMessage = (error: Unlock["error"] | null): string => {
  switch (error?.code) {
    case "WrongPassword":
      return "Wrong password.";
    case "NoEncryptedFile":
      return "This folder has no encrypted file to open.";
    default:
      return `Couldn't unlock this folder. ${error?.message}`;
  }
};

export const PasswordField = ({
  password,
  setPassword,
}: {
  password: string;
  setPassword: (password: string) => void;
}) => (
  <Input.Password
    className={styles.password}
    value={password}
    placeholder="Password"
    autoComplete="off"
    aria-label="Password"
    autoFocus
    onChange={(event) => setPassword(event.target.value)}
  />
);
