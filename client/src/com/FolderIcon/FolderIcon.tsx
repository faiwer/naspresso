import { isEncryptedFolder } from "../../api/storage";
import FolderIconSvg from "./folder.svg?react";
import FolderEncryptedIconSvg from "./folderEncrypted.svg?react";

export function FolderIcon({ name }: { name: string }) {
  return isEncryptedFolder(name) ? (
    <FolderEncryptedIconSvg />
  ) : (
    <FolderIconSvg />
  );
}
