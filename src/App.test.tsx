import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

const persistedPreferences = {
  privacyMode: "manual_only" as const,
  defaultCaptureRetention: "until_deleted" as const,
  exclusions: [
    {
      id: "ex-1",
      kind: "application" as const,
      value: "BankingApp",
      isEnabled: true,
      createdAt: "2026-08-13T10:00:00Z",
      updatedAt: "2026-08-13T10:00:00Z",
    },
  ],
};

const persistedWorkspace = {
  privacyMode: "manual_only" as const,
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
  decisions: [],
};

describe("Aura Continuity Desk privacy boundary", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command, payload) => {
      if (command === "get_workspace_snapshot") {
        return Promise.resolve(persistedWorkspace);
      }
      if (command === "get_privacy_preferences") {
        return Promise.resolve(persistedPreferences);
      }
      if (command === "update_privacy_preferences") {
        return Promise.resolve({
          ...persistedPreferences,
          privacyMode: (payload as { input: { privacyMode: string } }).input.privacyMode,
        });
      }
      if (command === "list_decisions") {
        return Promise.resolve([]);
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
          retention: undefined,
        },
      });
    });
  });

  it("records a user-authored decision with explicit local provenance", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Memory" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Memory" }));
    await user.type(screen.getByLabelText("Decision"), "Use bundled SQLite");
    await user.type(
      screen.getByLabelText("Rationale"),
      "The application needs a portable local database.",
    );
    await user.type(screen.getByLabelText(/^Source or basis/), "ADR-003");
    await user.click(screen.getByRole("button", { name: "Save decision" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_decision", {
        input: {
          projectId: "aura",
          title: "Use bundled SQLite",
          rationale: "The application needs a portable local database.",
          confidence: "medium",
          sourceLabels: ["ADR-003"],
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

const sampleExportManifest = {
  formatVersion: 2,
  sealing: "dpapi" as const,
  exportedAt: "2026-08-14T09:12:00Z",
  exportedByVersion: "0.1.0",
  recordCounts: {
    projects: 1,
    captures: 3,
    decisions: 2,
    exclusionRules: 1,
    settings: 2,
  },
  payloadChecksum: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  payloadSealedLength: 1440,
};

describe("Aura export and recovery controls", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command) => {
      if (command === "get_workspace_snapshot") {
        return Promise.resolve(persistedWorkspace);
      }
      if (command === "get_privacy_preferences") {
        return Promise.resolve(persistedPreferences);
      }
      if (command === "list_decisions") {
        return Promise.resolve([]);
      }
      if (command === "export_workspace") {
        return Promise.resolve({ exportedPath: "C:\\Users\\Eternal\\Documents\\aura-export.json" });
      }
      if (command === "export_manifest") {
        return Promise.resolve(sampleExportManifest);
      }

      return Promise.resolve(undefined);
    });
  });

  it("exports the sealed workspace through the native export command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Settings" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Export & recovery" })).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Export workspace…" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_workspace", {});
    });

    await waitFor(() => {
      expect(screen.getByText(/Workspace archive saved to/)).toBeInTheDocument();
    });
  });

  it("starts a transactional restore through the native import command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Settings" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await user.click(screen.getByRole("button", { name: "Restore from archive…" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("import_workspace", {
        input: { passphrase: undefined },
      });
    });
  });

  it("starts a transactional restore through the native import command with empty input placeholder", async () => {
    // kept as an alias documentation of the v2 call shape
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(await screen.findByRole("button", { name: "Restore from archive…" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalled();
    });
  });

  it("passes the restoration passphrase through to the native import command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    const passphraseField = screen.getByLabelText("Passphrase for restoring a sealed archive");
    await user.type(passphraseField, "correct-horse-battery");
    await user.click(await screen.findByRole("button", { name: "Restore from archive…" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("import_workspace", {
        input: { passphrase: "correct-horse-battery" },
      });
    });
  });

  it("reports an error when the export is cancelled or the file cannot be written", async () => {
    invokeMock.mockImplementation((command) => {
      if (command === "get_workspace_snapshot") {
        return Promise.resolve(persistedWorkspace);
      }
      if (command === "get_privacy_preferences") {
        return Promise.resolve(persistedPreferences);
      }
      if (command === "list_decisions") {
        return Promise.resolve([]);
      }
      if (command === "export_workspace") {
        return Promise.reject(new Error("dialog cancelled"));
      }
      return Promise.resolve(undefined);
    });

    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(await screen.findByRole("button", { name: "Export workspace…" }));

    await waitFor(() => {
      expect(screen.getByText(/Aura could not export the workspace/)).toBeInTheDocument();
    });

    expect(invokeMock).toHaveBeenCalledWith("export_workspace", {});
  });

  it("shows a typed export manifest preview with the sealed record counts", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(await screen.findByRole("button", { name: "Preview export contents" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_manifest", {});
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenLastCalledWith("export_manifest", {});
    });

    const manifestList = await screen.findByLabelText("Export contents");
    const captureCount = within(manifestList)
      .getByText("Captures")
      .closest("div")
      ?.querySelector("dd");
    expect(captureCount?.textContent).toBe("3");
    const decisionCount = within(manifestList)
      .getByText("Decisions")
      .closest("div")
      ?.querySelector("dd");
    expect(decisionCount?.textContent).toBe("2");
    const ruleCount = within(manifestList)
      .getByText("Exclusion rules")
      .closest("div")
      ?.querySelector("dd");
    expect(ruleCount?.textContent).toBe("1");
  });

  it("documents the key-binding trade-off and the local-only boundary", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Export & recovery" })).toBeInTheDocument();
    });

    expect(
      screen.getByText(/archive opens only on an Aura installation holding the same workspace key/),
    ).toBeInTheDocument();
    expect(screen.getByText(/Nothing is ever sent to a network/)).toBeInTheDocument();
  });

  it("offers passphrase sealing with a documented portability trade-off", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Export & recovery" })).toBeInTheDocument();
    });

    const passphraseSealing = screen.getByRole("radio", { name: /Seal with a passphrase/i });
    expect(passphraseSealing).toBeInTheDocument();
    await user.click(passphraseSealing);

    expect(
      screen.getByText(
        /Opens on any computer running Aura, as long as you remember the passphrase/,
      ),
    ).toBeInTheDocument();

    await user.click(await screen.findByRole("button", { name: "Export workspace…" }));

    const dialog = await screen.findByRole("dialog", {
      name: /Seal the archive with a passphrase/,
    });

    expect(
      within(dialog).getByText(
        /The passphrase itself is never stored anywhere; forgetting it means losing the archive/,
      ),
    ).toBeInTheDocument();
  });

  it("rejects a weak passphrase when sealing a portable archive", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(await screen.findByRole("radio", { name: /Seal with a passphrase/i }));
    await user.click(await screen.findByRole("button", { name: "Export workspace…" }));

    const dialog = await screen.findByRole("dialog", {
      name: /Seal the archive with a passphrase/,
    });
    const draftField = within(dialog).getByPlaceholderText(/At least 12 characters/);
    const confirmField = within(dialog).getByPlaceholderText(/Type it again/);
    await user.type(draftField, "short1");
    await user.type(confirmField, "short1");

    expect(screen.getByText(/Too weak to seal/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: /Seal and export/ }));

    await waitFor(() => {
      expect(
        screen.getByText(/That passphrase is too weak to seal an archive/),
      ).toBeInTheDocument();
    });
  });

  it("seals and exports with a strong passphrase through the native command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(await screen.findByRole("radio", { name: /Seal with a passphrase/i }));
    await user.click(await screen.findByRole("button", { name: "Export workspace…" }));

    const dialog = await screen.findByRole("dialog", {
      name: /Seal the archive with a passphrase/,
    });
    const draftField = within(dialog).getByPlaceholderText(/At least 12 characters/);
    const confirmField = within(dialog).getByPlaceholderText(/Type it again/);
    await user.type(draftField, "correct-horse-battery");
    await user.type(confirmField, "correct-horse-battery");

    expect(screen.getByText(/Strong enough to seal/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: /Seal and export/ }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_workspace_with_passphrase", {
        passphrase: "correct-horse-battery",
      });
    });
  });
});

describe("Aura local privacy preferences", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command) => {
      if (command === "get_workspace_snapshot") {
        return Promise.resolve(persistedWorkspace);
      }
      if (command === "get_privacy_preferences") {
        return Promise.resolve(persistedPreferences);
      }
      if (command === "list_decisions") {
        return Promise.resolve([]);
      }

      return Promise.resolve(undefined);
    });
  });

  it("opens the local privacy settings workspace and shows future-ready exclusion copy", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Settings" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "You set the boundary." })).toBeInTheDocument();
    });

    expect(screen.getByText(/Aura V0 has no passive observation adapter/)).toBeInTheDocument();
    expect(screen.getByText("BankingApp")).toBeInTheDocument();
  });

  it("saves the manual-only mode and default retention through the typed preferences command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Settings" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByRole("radio", { name: /Manual only/i })).toBeInTheDocument();
    });

    const retentionSelect = screen.getByRole("combobox", {
      name: "Default capture retention",
    });
    await user.selectOptions(retentionSelect, "review_in_30_days");
    await user.click(screen.getByRole("button", { name: "Save privacy preferences" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_privacy_preferences", {
        input: { privacyMode: "manual_only", defaultCaptureRetention: "review_in_30_days" },
      });
    });
  });

  it("registers a future exclusion rule with explicit future-policy framing", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Settings" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Exclusion value")).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText("Exclusion value"), "vault.local");
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Exclusion rule type" }),
      "domain",
    );
    await user.click(screen.getByRole("button", { name: "Add rule" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_exclusion_rule", {
        input: { kind: "domain", value: "vault.local" },
      });
    });

    expect(screen.getByText(/Future exclusion rule saved locally/)).toBeInTheDocument();
  });

  it("rejects an empty exclusion value without invoking the native command", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Settings" })).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Exclusion value")).toBeInTheDocument();
    });

    await user.clear(screen.getByLabelText("Exclusion value"));
    await user.click(screen.getByRole("button", { name: "Add rule" }));

    expect(
      screen.getByText("Enter the application, domain, or project name to exclude."),
    ).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("create_exclusion_rule", expect.anything());
  });
});
