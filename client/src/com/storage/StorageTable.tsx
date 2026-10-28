import { Skeleton, Table, type TableColumnsType } from "antd";
import { Link } from "react-router";

import { getEntryFileUrl } from "../../api/storage";
import { range } from "../../core/helpers/array";
import { formatDateTime } from "../../core/helpers/date";
import { getExtension } from "../../core/helpers/path";
import { routes } from "../../core/routes";
import type { StorageViewProps } from "./StorageList";

import styles from "./StorageTable.module.css";

export const StorageTable = ({
  segments,
  entries,
  skeletons,
}: StorageViewProps) => {
  const rows: Row[] = entries.map((entry) => ({
    key: entry.name,
    isSkeleton: false,
    name: entry.name,
    ext: entry.kind === "file" ? getExtension(entry.name) : "",
    createdAt: entry.createdAt,
    to: entry.kind === "dir" ? routes.storage([...segments, entry.name]) : null,
    href: entry.kind === "file" ? getEntryFileUrl(segments, entry.name) : null,
  }));
  for (const index of range(skeletons)) {
    rows.push({ ...SKELETON_ROW, key: `skeleton-${index}` });
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
  /** A placeholder for an entry that's still loading; the other fields are blank. */
  isSkeleton: boolean;
  name: string;
  ext: string;
  createdAt: number | null;
  /** In-app route of a directory; `null` for files. */
  to: string | null;
  /** URL of a file, opened in a new tab; `null` for directories. */
  href: string | null;
};

const SKELETON_ROW: Omit<Row, "key"> = {
  isSkeleton: true,
  name: "",
  ext: "",
  createdAt: null,
  to: null,
  href: null,
};

const NAME_SKELETON = <Skeleton.Input active size="small" />;
// Narrow columns get narrow bars: antd's default width would stretch them.
const EXT_SKELETON = (
  <Skeleton.Input active size="small" style={{ width: "3rem", minWidth: 0 }} />
);
const CREATED_SKELETON = (
  <Skeleton.Input active size="small" style={{ width: "8rem", minWidth: 0 }} />
);

const COLUMNS: TableColumnsType<Row> = [
  {
    title: "Name",
    dataIndex: "name",
    render: (name: string, row) => {
      if (row.isSkeleton) {
        return NAME_SKELETON;
      }
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
  {
    title: "Ext",
    dataIndex: "ext",
    width: 1,
    className: styles.shrink,
    render: (ext: string, row) => (row.isSkeleton ? EXT_SKELETON : ext),
  },
  {
    title: "Created",
    dataIndex: "createdAt",
    width: 1,
    className: styles.shrink,
    render: (createdAt: number | null, row) => {
      if (row.isSkeleton) {
        return CREATED_SKELETON;
      }
      return createdAt == null ? "—" : formatDateTime(createdAt);
    },
  },
];
