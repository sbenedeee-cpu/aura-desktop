import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";
import fs from "fs";
import path from "path";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

describe("Aura Frontend Privacy Boundary & Least Privilege Contracts", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command) => {
      if (command === "get_workspace_snapshot") {
        return Promise.resolve({
          activeProject: "Aura Desktop",
          continuityNote: "Scaffold active.",
          nextStep: "Verification step.",
          privacyMode: "focused",
          projects: [],
          signals: [],
        });
      }
      return Promise.resolve(undefined);
    });
  });

  it("never imports or invokes direct database libraries or raw SQL inside the renderer bundle", () => {
    // To ensure least-privilege, the frontend React components must never use SQL/SQLite or handle keys directly.
    // Let's verify that the source code of App.tsx has no direct database imports, raw SQL queries, or encryption key references.
    const appSourceCode = fs.readFileSync(path.join(__dirname, "App.tsx"), "utf8");

    // Check that we don't import standard db libs
    expect(appSourceCode).not.toContain("sqlite");
    expect(appSourceCode).not.toContain("rusqlite");
    expect(appSourceCode).not.toContain("sqlcipher");

    // Check that we don't write raw SQL queries
    expect(appSourceCode).not.toMatch(/SELECT\s+.*\s+FROM/i);
    expect(appSourceCode).not.toMatch(/INSERT\s+INTO/i);
    expect(appSourceCode).not.toMatch(/UPDATE\s+.*\s+SET/i);

    // Check that encryption/DPAPI key material is absent
    expect(appSourceCode).not.toContain("DPAPI");
    expect(appSourceCode).not.toContain("CryptProtectData");
    expect(appSourceCode).not.toContain("dek");
    expect(appSourceCode).not.toContain("encryption_key");
  });

  it("communicates solely via narrow, typed Tauri command boundaries", async () => {
    render(<App />);

    // Frontend should invoke get_workspace_snapshot immediately to load data
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_workspace_snapshot");
    });

    // Verify invokeMock was only called with approved API endpoints
    const allowedCommands = ["get_workspace_snapshot", "set_privacy_mode", "record_intentional_capture"];
    invokeMock.mock.calls.forEach((call) => {
      expect(allowedCommands).toContain(call[0]);
    });
  });

  it("does not expose any passive screen, audio, or clipboard recording APIs in the frontend codebase", () => {
    const appSourceCode = fs.readFileSync(path.join(__dirname, "App.tsx"), "utf8");

    // Ensure no background capture APIs are present in the frontend shell
    expect(appSourceCode).not.toContain("navigator.mediaDevices.getUserMedia");
    expect(appSourceCode).not.toContain("getDisplayMedia");
    expect(appSourceCode).not.toContain("navigator.clipboard.readText");
    expect(appSourceCode).not.toContain("clipboard.read");
  });
});
