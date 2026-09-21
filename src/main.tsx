import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

class ErrorBoundary extends React.Component<
  React.PropsWithChildren,
  { error: boolean }
> {
  state = { error: false };
  static getDerivedStateFromError() {
    return { error: true };
  }
  render() {
    return this.state.error ? (
      <div className="fatal">
        <h1>Let’s open Cricket again.</h1>
        <p>
          The interface hit an unexpected problem. Your transfer history is
          saved locally.
        </p>
        <button className="primary" onClick={() => location.reload()}>
          Reload Cricket
        </button>
      </div>
    ) : (
      this.props.children
    );
  }
}
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
