import { Link } from "react-router";

import styles from "./BreadcrumbItem.module.css";

export const BreadcrumbItem = ({
  Icon,
  label,
  to,
}: {
  Icon: React.ComponentType<{ className?: string }> | null;
  label: string;
  to?: string;
}) => {
  const content = (
    <>
      {Icon && <Icon className={styles.icon} />}
      {label}
    </>
  );
  return to ? (
    <Link className={styles.crumb} to={to}>
      {content}
    </Link>
  ) : (
    <span className={styles.crumb}>{content}</span>
  );
};
