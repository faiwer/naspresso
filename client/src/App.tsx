import { BrowserRouter, Route, Routes } from "react-router";

import { HomePage } from "./pages/HomePage";
import { StoragePage } from "./pages/StoragePage";

export const App = () => (
  <BrowserRouter>
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/storage/*" element={<StoragePage />} />
    </Routes>
  </BrowserRouter>
);
