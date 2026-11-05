import { DatabaseFilled, LockFilled, UnlockFilled } from "@ant-design/icons";

import type { Encryption } from "../../api/storage.io";
import { routes } from "../../core/routes";
import { BreadcrumbItem } from "../layout/BreadcrumbItem";

type Props = {
  segment: string;
  encryption: Encryption;
  isLast: boolean;
  to: string;
};

export const SegmentBreadcrumb = ({
  segment,
  encryption,
  isLast,
  to,
}: Props) => {
  return (
    <BreadcrumbItem
      Icon={
        isEncryptedFolder(segment)
          ? encryption === "locked"
            ? LockFilled
            : UnlockFilled
          : null
      }
      label={segment}
      to={isLast ? to : undefined}
    />
  );
};

const isEncryptedFolder = (segment: string) => segment.endsWith(".nped");

export const makeStorageBreadcrumb = (segments: string[]) => ({
  title: (
    <BreadcrumbItem
      Icon={DatabaseFilled}
      label="Storage"
      to={segments.length > 0 ? routes.storage() : undefined}
    />
  ),
});

export const makeSegmentBreadcrumbs = (
  segments: string[],
  encryption: Encryption,
) => {
  return [
    makeStorageBreadcrumb(segments),
    ...[...segments].map((label, index) => ({
      title: (
        <SegmentBreadcrumb
          key={index}
          segment={label}
          encryption={encryption}
          isLast={index === segments.length - 2}
          to={routes.storage(segments.slice(0, index))}
        />
      ),
    })),
  ];
};
