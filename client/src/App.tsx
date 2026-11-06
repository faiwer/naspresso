import { App as AntdApp } from "antd";
import { BrowserRouter, Route, Routes } from "react-router";

import { PreviewModalProvider } from "./com/previewModal/PreviewModalProvider";
import { HomePage } from "./pages/HomePage";
import { StoragePage } from "./pages/StoragePage";

export const App = () => (
  <AntdApp>
    <BrowserRouter>
      <PreviewModalProvider>
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/storage/*" element={<StoragePage />} />
        </Routes>
      </PreviewModalProvider>
    </BrowserRouter>
  </AntdApp>
);
