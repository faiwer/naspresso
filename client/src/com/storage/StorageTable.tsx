import { Table, type TableColumnsType } from "antd";
import { Link } from "react-router";

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
  }));
  if (segments.length > 0) {
    rows.unshift({
      key: "..",
      name: "..",
      ext: "",
      createdAt: null,
      to: routes.storage(segments.slice(0, -1)),
    });
  }

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
  /** Where clicking the name leads; `null` for files. */
  to: string | null;
};

const COLUMNS: TableColumnsType<Row> = [
  {
    title: "Name",
    dataIndex: "name",
    render: (name: string, row) =>
      row.to == null ? name : <Link to={row.to}>{name}</Link>,
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
