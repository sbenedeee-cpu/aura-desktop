import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

describe("Aura privacy boundary", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command) => {
      if (command === "get_workspace_snapshot") {
        return Promise.reject(new Error("Native command layer unavailable in renderer test"));
      }

      return Promise.resolve(undefined);
    });
  });

  it("blocks intentional capture while paused and does not invoke the capture command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Pause capture" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Pause capture" }));

    expect(screen.getByText("Capture paused")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Resume capture" })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "Add context" }));

    expect(
      screen.getByText("Resume intentional capture before adding context"),
    ).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("record_intentional_capture", expect.anything());
  });
});
