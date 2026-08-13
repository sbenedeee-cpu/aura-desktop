import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

const persistedWorkspace = {
  privacyMode: "focused" as const,
  selectedProject: {
    id: "aura",
    name: "Aura Desktop",
    goal: "Ship a trustworthy local-first desktop app",
    status: "active" as const,
    currentTask: "Refine the continuity desk",
    blocker: null,
    nextStep: "Review the project brief",
    createdAt: "2026-08-13T10:00:00Z",
    updatedAt: "2026-08-13T10:00:00Z",
    archivedAt: null,
  },
  projects: [
    {
      id: "aura",
      name: "Aura Desktop",
      status: "Active",
      nextStep: "Review the project brief",
      updatedAt: "2026-08-13T10:00:00Z",
      isSelected: true,
    },
    {
      id: "eternal",
      name: "Eternal Studios",
      status: "Active",
      nextStep: null,
      updatedAt: "2026-08-12T10:00:00Z",
      isSelected: false,
    },
  ],
  activity: [],
};

describe("Aura Continuity Desk privacy boundary", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command) => {
      if (command === "get_workspace_snapshot") {
        return Promise.resolve(persistedWorkspace);
      }

      return Promise.resolve(undefined);
    });
  });

  it("blocks explicit capture while paused and never invokes the capture command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Pause manual context" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Pause manual context" }));
    await user.click(screen.getByRole("button", { name: "Capture" }));

    expect(screen.getByText("Manual capture is paused.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Review before saving/i })).toBeDisabled();
    expect(invokeMock).not.toHaveBeenCalledWith("create_manual_capture", expect.anything());
  });

  it("requires review then sends only the typed capture draft to the native command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Capture" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Capture" }));
    await user.type(screen.getByLabelText("Label"), "Architecture decision");
    await user.type(screen.getByLabelText(/^Content/), "Use a Rust-owned persistence boundary.");
    await user.click(screen.getByRole("button", { name: /Review before saving/i }));

    expect(
      screen.getByText("Here is the exact local record Aura will create."),
    ).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("create_manual_capture", expect.anything());

    await user.click(screen.getByRole("button", { name: /Confirm and save locally/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_manual_capture", {
        input: {
          projectId: "aura",
          kind: "manual_note",
          label: "Architecture decision",
          content: "Use a Rust-owned persistence boundary.",
          classification: "standard",
          retention: "until_deleted",
        },
      });
    });
  });

  it("uses the typed selected-project command when a different local project is chosen", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Projects" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Projects" }));
    await user.click(screen.getByRole("button", { name: /Eternal Studios/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("select_project", { projectId: "eternal" });
    });
  });
});
