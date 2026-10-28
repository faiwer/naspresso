import { createRoot } from "react-dom/client";

import { App } from "./App";

import "./global.css";

createRoot(
  document.getElementById("root")!,
  import.meta.env.DEV
    ? // Vite eliminates this branch in prod.
      {
        testMode: true,
        transformSource: (source) => ({
          ...source,
          fileName: source.fileName.replace(
            /^.+\/src/,
            location.origin + "/src",
          ),
        }),
      }
    : undefined,
).render(<App />);
