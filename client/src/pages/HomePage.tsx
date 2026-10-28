import { getHealth } from "../api/health";
import { Frame } from "../com/layout/Frame";
import { useAsyncData } from "../core/hooks/useAsyncData";

export const HomePage = () => {
  const health = useAsyncData((signal) => getHealth({ signal }), []);
  const status = health.isLoading
    ? "…"
    : health.error
      ? "unreachable"
      : health.data.status;

  return (
    <Frame>
      <h1>Hello World, server is: {status}</h1>
    </Frame>
  );
};
