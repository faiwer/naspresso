import { Table, type TableColumnsType } from "antd";
import { Link } from "react-router";

import { getEntryFileUrl } from "../../api/storage";
import { formatDateTime } from "../../core/helpers/date";
import { getExtension } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageTable.module.css";

export const StorageTable = ({ segments, entries }: StorageViewProps) => {
  const rows: Row[] = entries.map((entry) => ({
    key: entry.name,
    name: entry.name,
    ext: entry.kind === "file" ? getExtension(entry.name) : "",
    createdAt: entry.createdAt,
    to: entry.kind === "dir" ? routes.storage([...segments, entry.name]) : null,
    href: entry.kind === "file" ? getEntryFileUrl(segments, entry.name) : null,
  }));

  return (
    <Table<Row>
      columns={COLUMNS}
      dataSource={rows}
      pagination={false}
      size="small"
    />
  );
};

type Row = {
  key: string;
  name: string;
  ext: string;
  createdAt: number | null;
  /** In-app route of a directory; `null` for files. */
  to: string | null;
  /** URL of a file, opened in a new tab; `null` for directories. */
  href: string | null;
};

const COLUMNS: TableColumnsType<Row> = [
  {
    title: "Name",
    dataIndex: "name",
    render: (name: string, row) => {
      if (row.to != null) {
        return <Link to={row.to}>{name}</Link>;
      }
      if (row.href != null) {
        return (
          <a href={row.href} target="_blank" rel="noreferrer">
            {name}
          </a>
        );
      }
      return name;
    },
  },
  { title: "Ext", dataIndex: "ext", width: 1, className: styles.shrink },
  {
    title: "Created",
    dataIndex: "createdAt",
    width: 1,
    className: styles.shrink,
    render: (createdAt: number | null) =>
      createdAt == null ? "—" : formatDateTime(createdAt),
  },
];
