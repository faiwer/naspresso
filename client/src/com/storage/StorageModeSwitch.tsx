import {
  AppstoreOutlined,
  BlockOutlined,
  TableOutlined,
  UnorderedListOutlined,
} from "@ant-design/icons";
import { Segmented } from "antd";

export type StorageViewMode = "list" | "masonry" | "table" | "thumbnails";

type StorageModeSwitchProps = {
  value: StorageViewMode;
  onChange: (mode: StorageViewMode) => void;
};

export const StorageModeSwitch = ({
  value,
  onChange,
}: StorageModeSwitchProps) => (
  <Segmented<StorageViewMode>
    value={value}
    onChange={onChange}
    options={[
      {
        value: "thumbnails",
        icon: <AppstoreOutlined />,
        tooltip: "Thumbnails",
      },
      {
        value: "masonry",
        icon: <BlockOutlined />,
        tooltip: "Masonry",
      },
      { value: "list", icon: <UnorderedListOutlined />, tooltip: "List" },
      { value: "table", icon: <TableOutlined />, tooltip: "Table" },
    ]}
  />
);
