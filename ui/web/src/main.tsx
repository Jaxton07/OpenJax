import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "markstream-react/index.css";
import "./styles/app.css";
import "./styles/settings.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
