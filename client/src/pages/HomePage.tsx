import { useEffect, useState } from "react";

export const HomePage = () => {
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    void fetchHealthStatus().then(setStatus);
  }, []);

  return <h1>Hello World, server is: {status ?? "…"}</h1>;
};

const fetchHealthStatus = async (): Promise<string> => {
  const response = await fetch("/api/health");
  const { status } = (await response.json()) as { status: string };
  return status;
};
