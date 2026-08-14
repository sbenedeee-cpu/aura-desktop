import { invoke } from "@tauri-apps/api/core";
import { FormEvent, ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";

type PrivacyMode = "manual_only" | "paused";
type View = "today" | "projects" | "capture" | "memory" | "settings";

type ProjectStatus = "active" | "paused" | "archived";

type Project = {
  id: string;
  name: string;
  goal: string | null;
  status: ProjectStatus;
  currentTask: string | null;
  blocker: string | null;
  nextStep: string | null;
  createdAt: string;
  updatedAt: string;
  archivedAt: string | null;
};

type ProjectListItem = {
  id: string;
  name: string;
  status: string;
  nextStep: string | null;
  updatedAt: string;
  isSelected: boolean;
};

type WorkspaceSignal = {
  id: string;
  kind: "context" | "project" | "decision" | "system";
  title: string;
  detail: string;
  time: string;
};

type DecisionSummary = {
  id: string;
  title: string;
  confidence: "low" | "medium" | "high";
  status: "confirmed" | "superseded";
  createdAt: string;
};

type DecisionClaim = DecisionSummary & {
  projectId: string;
  rationale: string;
  authorType: "user";
  updatedAt: string;
  supersedesClaimId: string | null;
  supersededByClaimId: string | null;
  sources: { id: string; label: string; createdAt: string }[];
};

type DecisionDraft = {
  projectId: string;
  title: string;
  rationale: string;
  confidence: DecisionSummary["confidence"];
  sourceLabels: string;
};

type WorkspaceSnapshot = {
  privacyMode: PrivacyMode;
  selectedProject: Project | null;
  projects: ProjectListItem[];
  activity: WorkspaceSignal[];
  decisions: DecisionSummary[];
};

type ProjectDraft = {
  name: string;
  goal: string;
  currentTask: string;
  blocker: string;
  nextStep: string;
};

type CaptureKind = "manual_note" | "pasted_text" | "url";
type CaptureClassification = "standard" | "sensitive";
type CaptureRetention = "until_deleted" | "review_in_30_days";

type CaptureDraft = {
  projectId: string;
  kind: CaptureKind;
  label: string;
  content: string;
  classification: CaptureClassification;
  retention?: CaptureRetention;
};

type ExclusionKind = "application" | "domain" | "project";

type KeyVaultStatus = {
  wrappedKeyPersisted: boolean;
  keyLength: number;
  sealedVersion: number;
};
type ExclusionRule = {
  id: string;
  kind: ExclusionKind;
  value: string;
  isEnabled: boolean;
  createdAt: string;
  updatedAt: string;
};

type PrivacyPreferences = {
  privacyMode: PrivacyMode;
  defaultCaptureRetention: CaptureRetention;
  exclusions: ExclusionRule[];
};

type ExportRecordCounts = {
  projects: number;
  captures: number;
  decisions: number;
  exclusionRules: number;
  settings: number;
};

type ExportManifest = {
  formatVersion: number;
  exportedAt: string;
  exportedByVersion: string;
  recordCounts: ExportRecordCounts;
  payloadChecksum: string;
  payloadSealedLength: number;
};

const emptySnapshot: WorkspaceSnapshot = {
  privacyMode: "manual_only",
  selectedProject: null,
  projects: [],
  activity: [],
  decisions: [],
};

const navigation = [
  { id: "today" as const, label: "Today", icon: "sun" },
  { id: "projects" as const, label: "Projects", icon: "folder" },
  { id: "capture", label: "Capture", icon: "plus" },
  { id: "memory", label: "Memory", icon: "book" },
  { id: "activity", label: "Activity", icon: "clock" },
  { id: "settings", label: "Settings", icon: "settings" },
];

function Icon({ name }: { name: string }) {
  const common = {
    fill: "none",
    stroke: "currentColor",
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    strokeWidth: 1.7,
  };

  const paths: Record<string, ReactNode> = {
    sun: (
      <>
        <circle cx="12" cy="12" r="3.6" {...common} />
        <path
          d="M12 2.5v2M12 19.5v2M21.5 12h-2M4.5 12h-2M18.7 5.3l-1.4 1.4M6.7 17.3l-1.4 1.4M18.7 18.7l-1.4-1.4M6.7 6.7L5.3 5.3"
          {...common}
        />
      </>
    ),
    folder: (
      <path
        d="M3.5 6.5h6l1.8 2h9.2v9.2a2 2 0 0 1-2 2h-15a2 2 0 0 1-2-2V8.5a2 2 0 0 1 2-2Z"
        {...common}
      />
    ),
    plus: <path d="M12 5v14M5 12h14" {...common} />,
    book: (
      <path
        d="M4.5 4.5A3.5 3.5 0 0 1 8 4h11.5v15.5H8a3.5 3.5 0 0 0-3.5.5V4.5ZM4.5 4.5V20"
        {...common}
      />
    ),
    clock: (
      <>
        <circle cx="12" cy="12" r="8.5" {...common} />
        <path d="M12 7v5l3.2 2" {...common} />
      </>
    ),
    settings: (
      <>
        <circle cx="12" cy="12" r="3" {...common} />
        <path
          d="m19 13.5 1.2 1.3-2.1 3.6-1.7-.7a8 8 0 0 1-1.8 1l-.3 1.8h-4.1l-.3-1.8a8 8 0 0 1-1.8-1l-1.7.7-2.1-3.6L5.5 13.5a8 8 0 0 1 0-2.1L4.3 10l2.1-3.6 1.7.7a8 8 0 0 1 1.8-1l.3-1.8h4.1l.3 1.8a8 8 0 0 1 1.8 1l1.7-.7 2.1 3.6-1.2 1.4a8 8 0 0 1 0 2.1Z"
          {...common}
        />
      </>
    ),
    arrow: <path d="M5 12h13M13 7l5 5-5 5" {...common} />,
    shield: (
      <>
        <path d="M12 3.3 19 6v5.3c0 4.3-2.9 7.7-7 9.4-4.1-1.7-7-5.1-7-9.4V6l7-2.7Z" {...common} />
        <path d="m9.2 12 1.8 1.8 3.8-4" {...common} />
      </>
    ),
    pencil: (
      <>
        <path d="m4.5 16.8-.8 3.5 3.5-.8L18.5 8.2l-2.7-2.7L4.5 16.8Z" {...common} />
        <path d="m14.5 6.8 2.7 2.7" {...common} />
      </>
    ),
    archive: (
      <>
        <path d="M4 7h16v13H4zM3 4h18v3H3zM9 11h6" {...common} />
      </>
    ),
    chevron: <path d="m9 7 5 5-5 5" {...common} />,
    lock: (
      <>
        <rect height="10" rx="1.8" width="13" x="5.5" y="10" {...common} />
        <path d="M8 10V7.8a4 4 0 0 1 8 0V10M12 14v2" {...common} />
      </>
    ),
  };

  return (
    <svg aria-hidden="true" className="icon" viewBox="0 0 24 24">
      {paths[name]}
    </svg>
  );
}

function valueOrEmpty(value: string | null) {
  return value?.trim() || "";
}

function formatTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(date);
}

function privacyCopy(mode: PrivacyMode) {
  return mode === "manual_only"
    ? "Manual only — no background capture"
    : "Paused — no context is collected or queued";
}

function retentionLabel(retention: CaptureRetention) {
  return retention === "until_deleted" ? "Until deleted" : "Review in 30 days";
}

function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot>(emptySnapshot);
  const [activeView, setActiveView] = useState<View>("today");
  const [isLoading, setIsLoading] = useState(true);
  const [isSavingPrivacy, setIsSavingPrivacy] = useState(false);
  const [isSavingCapture, setIsSavingCapture] = useState(false);
  const [isSelectingProject, setIsSelectingProject] = useState(false);
  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [isSavingProject, setIsSavingProject] = useState(false);
  const [isArchivingProject, setIsArchivingProject] = useState(false);
  const [showCreateProject, setShowCreateProject] = useState(false);
  const [showArchiveDialog, setShowArchiveDialog] = useState(false);
  const [notice, setNotice] = useState("");
  const [loadError, setLoadError] = useState("");
  const [decisions, setDecisions] = useState<DecisionClaim[]>([]);
  const [isLoadingDecisions, setIsLoadingDecisions] = useState(false);
  const [isSavingDecision, setIsSavingDecision] = useState(false);
  const [decisionError, setDecisionError] = useState("");
  const [privacyPreferences, setPrivacyPreferences] = useState<PrivacyPreferences | null>(null);
  const [isLoadingPreferences, setIsLoadingPreferences] = useState(false);
  const [isSavingPreferences, setIsSavingPreferences] = useState(false);
  const [settingsError, setSettingsError] = useState("");
  const [exportError, setExportError] = useState("");
  const [exportNotice, setExportNotice] = useState("");
  const [exportBusy, setExportBusy] = useState(false);
  const [exportManifest, setExportManifest] = useState<ExportManifest | null>(null);

  const selectedProject = snapshot.selectedProject;

  const dismissNotice = useCallback(() => {
    window.setTimeout(() => setNotice(""), 3600);
  }, []);

  const loadWorkspace = useCallback(async () => {
    setIsLoading(true);
    setLoadError("");

    try {
      const loaded = await invoke<WorkspaceSnapshot>("get_workspace_snapshot");
      setSnapshot(loaded);
    } catch {
      setSnapshot(emptySnapshot);
      setLoadError(
        "Aura could not open the local workspace. Your existing records were not changed. Try again or restart Aura to recover the local connection.",
      );
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    const deferredLoad = window.setTimeout(() => {
      void loadWorkspace();
    }, 0);

    return () => window.clearTimeout(deferredLoad);
  }, [loadWorkspace]);

  const loadPrivacyPreferences = useCallback(async () => {
    setIsLoadingPreferences(true);
    setSettingsError("");
    try {
      const preferences = await invoke<PrivacyPreferences>("get_privacy_preferences");
      setPrivacyPreferences(preferences);
    } catch {
      setPrivacyPreferences(null);
      setSettingsError(
        "Aura could not load local privacy preferences. Existing records were not changed.",
      );
    } finally {
      setIsLoadingPreferences(false);
    }
  }, []);

  useEffect(() => {
    if (activeView !== "settings") {
      return;
    }
    const deferredLoad = window.setTimeout(() => {
      void loadPrivacyPreferences();
    }, 0);
    return () => window.clearTimeout(deferredLoad);
  }, [activeView, loadPrivacyPreferences]);

  const loadDecisions = useCallback(async (projectId: string) => {
    setIsLoadingDecisions(true);
    setDecisionError("");
    try {
      const records = await invoke<DecisionClaim[]>("list_decisions", { projectId });
      setDecisions(records);
    } catch {
      setDecisions([]);
      setDecisionError("Aura could not load this project’s local decision record.");
    } finally {
      setIsLoadingDecisions(false);
    }
  }, []);

  useEffect(() => {
    if (activeView !== "memory" || !selectedProject) {
      return;
    }
    const deferredLoad = window.setTimeout(() => {
      void loadDecisions(selectedProject.id);
    }, 0);
    return () => window.clearTimeout(deferredLoad);
  }, [activeView, loadDecisions, selectedProject]);

  const continuity = useMemo(
    () => [
      { label: "Goal", value: selectedProject?.goal || "No goal recorded" },
      { label: "Current task", value: selectedProject?.currentTask || "No current task recorded" },
      { label: "Next step", value: selectedProject?.nextStep || "No next step recorded" },
      { label: "Blocker", value: selectedProject?.blocker || "No blocker recorded" },
    ],
    [selectedProject],
  );

  async function savePrivacyPreferences(
    privacyMode: PrivacyMode,
    defaultCaptureRetention: CaptureRetention,
    source: "sidebar" | "settings" = "settings",
  ) {
    if (source === "sidebar") {
      setIsSavingPrivacy(true);
    } else {
      setIsSavingPreferences(true);
      setSettingsError("");
    }

    try {
      const preferences = await invoke<PrivacyPreferences>("update_privacy_preferences", {
        input: { privacyMode, defaultCaptureRetention },
      });

      if (!preferences) {
        throw new Error("Aura received an empty preferences record after saving locally.");
      }

      setPrivacyPreferences(preferences);
      setSnapshot((current) => ({ ...current, privacyMode: preferences.privacyMode }));
      setNotice(
        preferences.privacyMode === "manual_only"
          ? "Manual-only capture is ready locally"
          : "Manual capture is paused locally",
      );
      return true;
    } catch {
      const message = "Aura could not update the local privacy preferences.";
      if (source === "settings") {
        setSettingsError(message);
      } else {
        setNotice(message);
      }
      return false;
    } finally {
      if (source === "sidebar") {
        setIsSavingPrivacy(false);
      } else {
        setIsSavingPreferences(false);
      }
      dismissNotice();
    }
  }

  async function togglePrivacyMode() {
    const nextMode: PrivacyMode = snapshot.privacyMode === "manual_only" ? "paused" : "manual_only";
    await savePrivacyPreferences(
      nextMode,
      privacyPreferences?.defaultCaptureRetention || "until_deleted",
      "sidebar",
    );
  }

  async function exportWorkspace() {
    setExportBusy(true);
    setExportError("");
    setExportNotice("");
    setExportManifest(null);
    try {
      const result = await invoke<{ exportedPath: string }>("export_workspace", {});
      setExportNotice(
        result?.exportedPath
          ? `Workspace archive saved to ${result.exportedPath}`
          : "Workspace archive saved locally. Keep the file safe; it can only be opened by an Aura installation with the same workspace key.",
      );
    } catch {
      setExportError(
        "Aura could not export the workspace. The export was cancelled or the file could not be written.",
      );
    } finally {
      setExportBusy(false);
    }
  }

  async function importWorkspace() {
    setExportBusy(true);
    setExportError("");
    setExportNotice("");
    try {
      await invoke<null>("import_workspace", {});
      setExportNotice(
        "Workspace archive restored. Aura reloaded its local projects, captures, and decisions.",
      );
      await loadPrivacyPreferences();
      setSnapshot(await invoke<WorkspaceSnapshot>("get_workspace_snapshot"));
    } catch {
      setExportError(
        "Aura could not restore that archive. It may be damaged, use an unsupported format, or belong to a different workspace key.",
      );
    } finally {
      setExportBusy(false);
    }
  }

  async function showExportManifest() {
    setExportError("");
    setExportManifest(null);
    try {
      const manifest = await invoke<ExportManifest>("export_manifest", {});
      setExportManifest(manifest);
    } catch {
      setExportError("Aura could not prepare the export preview.");
    }
  }

  async function addExclusion(kind: ExclusionKind, value: string) {
    setIsSavingPreferences(true);
    setSettingsError("");
    try {
      await invoke<ExclusionRule>("create_exclusion_rule", { input: { kind, value } });
      await loadPrivacyPreferences();
      setNotice("Future exclusion rule saved locally. No observation adapter is active.");
      return true;
    } catch {
      setSettingsError("Aura could not save that local exclusion rule.");
      return false;
    } finally {
      setIsSavingPreferences(false);
      dismissNotice();
    }
  }

  async function changeExclusionState(exclusionId: string, isEnabled: boolean) {
    setIsSavingPreferences(true);
    setSettingsError("");
    try {
      await invoke<ExclusionRule>("set_exclusion_enabled", {
        exclusionId,
        input: { isEnabled },
      });
      await loadPrivacyPreferences();
    } catch {
      setSettingsError("Aura could not update that local exclusion rule.");
    } finally {
      setIsSavingPreferences(false);
    }
  }

  async function selectProject(projectId: string) {
    if (selectedProject?.id === projectId) {
      return;
    }

    setIsSelectingProject(true);
    try {
      await invoke("select_project", { projectId });
      await loadWorkspace();
    } catch {
      setNotice("Aura could not select that local project");
      dismissNotice();
    } finally {
      setIsSelectingProject(false);
    }
  }

  async function createManualCapture(draft: CaptureDraft) {
    if (snapshot.privacyMode === "paused") {
      setNotice("Resume manual context before saving a capture");
      dismissNotice();
      return false;
    }

    setIsSavingCapture(true);
    try {
      await invoke("create_manual_capture", { input: draft });
      await loadWorkspace();
      setNotice("Capture saved locally. Aura made no network request.");
      return true;
    } catch {
      setNotice("Aura could not save this capture. Its content is still only in the form.");
      return false;
    } finally {
      setIsSavingCapture(false);
      dismissNotice();
    }
  }

  async function saveDecision(draft: DecisionDraft, correctedDecisionId?: string) {
    if (!draft.projectId) {
      setDecisionError("Choose a local project before recording a decision.");
      return false;
    }
    const sourceLabels = draft.sourceLabels
      .split("\n")
      .map((label) => label.trim())
      .filter(Boolean);
    setIsSavingDecision(true);
    setDecisionError("");
    try {
      const input = { ...draft, sourceLabels };
      if (correctedDecisionId) {
        await invoke<DecisionClaim>("correct_decision", { decisionId: correctedDecisionId, input });
        setNotice("Correction saved locally. The earlier decision remains visible as superseded.");
      } else {
        await invoke<DecisionClaim>("create_decision", { input });
        setNotice("Decision saved locally with your stated basis.");
      }
      await Promise.all([loadWorkspace(), loadDecisions(draft.projectId)]);
      return true;
    } catch {
      setDecisionError("Aura could not save this local decision. Review the fields and try again.");
      return false;
    } finally {
      setIsSavingDecision(false);
      dismissNotice();
    }
  }

  async function createProject(draft: Pick<ProjectDraft, "name" | "goal" | "nextStep">) {
    setIsCreatingProject(true);
    try {
      await invoke<Project>("create_project", {
        input: {
          name: draft.name,
          goal: draft.goal,
          nextStep: draft.nextStep,
        },
      });
      setShowCreateProject(false);
      await loadWorkspace();
      setActiveView("projects");
      setNotice("Project saved locally");
    } catch {
      setNotice("Aura could not save the local project");
    } finally {
      setIsCreatingProject(false);
      dismissNotice();
    }
  }

  async function saveProject(projectId: string, draft: ProjectDraft) {
    setIsSavingProject(true);
    try {
      await invoke<Project>("update_project", { projectId, input: draft });
      await loadWorkspace();
      setNotice("Project saved locally");
    } catch {
      setNotice("Aura could not save your changes. They are still in this form.");
      throw new Error("project-save-failed");
    } finally {
      setIsSavingProject(false);
      dismissNotice();
    }
  }

  async function archiveSelectedProject() {
    if (!selectedProject) {
      return;
    }

    setIsArchivingProject(true);
    try {
      await invoke("archive_project", { projectId: selectedProject.id });
      setShowArchiveDialog(false);
      await loadWorkspace();
      setNotice(`${selectedProject.name} was archived locally`);
    } catch {
      setNotice("Aura could not archive the local project");
    } finally {
      setIsArchivingProject(false);
      dismissNotice();
    }
  }

  const heading =
    activeView === "today"
      ? "Continue with clarity."
      : activeView === "projects"
        ? "Projects"
        : activeView === "capture"
          ? "Capture deliberately."
          : activeView === "memory"
            ? "Decision memory."
            : "Privacy settings.";
  const subheading =
    activeView === "today"
      ? "Resume one deliberate thread at a time."
      : activeView === "projects"
        ? "Local projects, held in context."
        : activeView === "capture"
          ? "Review exactly what Aura will keep before you save it."
          : activeView === "memory"
            ? "Record what was decided, why, and what later replaced it."
            : "Choose how deliberate local capture behaves on this device.";

  return (
    <div className="continuity-desk">
      <aside className="desk-nav" aria-label="Aura navigation">
        <button className="wordmark" onClick={() => setActiveView("today")} type="button">
          <span aria-hidden="true" className="wordmark-mark">
            a
          </span>
          <span>Aura</span>
        </button>

        <nav className="route-list" aria-label="Workspace routes">
          {navigation.map((item) => {
            const isAvailable =
              item.id === "today" ||
              item.id === "projects" ||
              item.id === "capture" ||
              item.id === "memory" ||
              item.id === "settings";
            const isActive = activeView === item.id;
            return (
              <button
                aria-current={isActive ? "page" : undefined}
                aria-disabled={!isAvailable}
                className={`route-button ${isActive ? "is-active" : ""} ${!isAvailable ? "is-planned" : ""}`}
                disabled={!isAvailable}
                key={item.id}
                onClick={() => isAvailable && setActiveView(item.id as View)}
                type="button"
              >
                <Icon name={item.icon} />
                <span>{item.label}</span>
                {!isAvailable && <small>Planned</small>}
              </button>
            );
          })}
        </nav>

        <div className="privacy-panel">
          <div className="privacy-panel-heading">
            <Icon name="shield" />
            <span>{snapshot.privacyMode === "manual_only" ? "Manual only" : "Paused"}</span>
          </div>
          <p>{privacyCopy(snapshot.privacyMode)}</p>
          <button
            className="quiet-action"
            disabled={isSavingPrivacy}
            onClick={togglePrivacyMode}
            type="button"
          >
            {isSavingPrivacy
              ? "Updating…"
              : snapshot.privacyMode === "manual_only"
                ? "Pause manual context"
                : "Resume manual context"}
          </button>
        </div>
        <p className="build-stamp">AURA DESKTOP · LOCAL V0</p>
      </aside>

      <main className="desk-main">
        <header className="desk-header">
          <div>
            <p className="route-eyebrow">
              {activeView === "today"
                ? "TODAY"
                : activeView === "projects"
                  ? "PROJECTS"
                  : activeView === "capture"
                    ? "CAPTURE"
                    : activeView === "memory"
                      ? "MEMORY"
                      : "SETTINGS"}
            </p>
            <h1>{heading}</h1>
            <p className="header-subtitle">{subheading}</p>
          </div>
          <div className="header-actions">
            <span className="local-status">
              <span aria-hidden="true" />
              Local only
            </span>
            {activeView === "projects" ? (
              <button
                className="primary-action"
                onClick={() => setShowCreateProject(true)}
                type="button"
              >
                <Icon name="plus" />
                New project
              </button>
            ) : activeView === "today" ? (
              <button
                className="primary-action"
                onClick={() => setActiveView("capture")}
                type="button"
              >
                <Icon name="plus" />
                Add manual capture
              </button>
            ) : null}
          </div>
        </header>

        {notice && (
          <div className="desk-notice" role="status">
            {notice}
          </div>
        )}
        {loadError && (
          <div className="desk-notice is-error" role="alert">
            <span>{loadError}</span>
            <button className="text-link" onClick={() => void loadWorkspace()} type="button">
              Try again
            </button>
          </div>
        )}

        {isLoading ? (
          <LoadingDesk />
        ) : activeView === "today" ? (
          <TodayView
            activity={snapshot.activity}
            continuity={continuity}
            onOpenProject={() => setActiveView("projects")}
            onSelectProject={selectProject}
            privacyMode={snapshot.privacyMode}
            projectOptions={snapshot.projects}
            selectedProject={selectedProject}
            selectionBusy={isSelectingProject}
          />
        ) : activeView === "projects" ? (
          <ProjectsView
            isSaving={isSavingProject}
            onArchive={() => setShowArchiveDialog(true)}
            onCreate={() => setShowCreateProject(true)}
            onSave={saveProject}
            onSelectProject={selectProject}
            projects={snapshot.projects}
            selectedProject={selectedProject}
            selectionBusy={isSelectingProject}
          />
        ) : activeView === "capture" ? (
          <CaptureView
            defaultRetention={privacyPreferences?.defaultCaptureRetention || "until_deleted"}
            isSaving={isSavingCapture}
            onCancel={() => setActiveView("today")}
            onSave={createManualCapture}
            onSelectProject={selectProject}
            privacyMode={snapshot.privacyMode}
            projectOptions={snapshot.projects}
            selectedProject={selectedProject}
            selectionBusy={isSelectingProject}
          />
        ) : activeView === "memory" ? (
          <MemoryView
            key={selectedProject?.id || "no-project"}
            decisions={decisions}
            error={decisionError}
            isLoading={isLoadingDecisions}
            isSaving={isSavingDecision}
            onSave={saveDecision}
            project={selectedProject}
            projectOptions={snapshot.projects}
          />
        ) : (
          <SettingsView
            error={settingsError}
            exportError={exportError}
            exportBusy={exportBusy}
            exportManifest={exportManifest}
            exportNotice={exportNotice}
            isLoading={isLoadingPreferences}
            isSaving={isSavingPreferences}
            onAddExclusion={addExclusion}
            onExportWorkspace={exportWorkspace}
            onImportWorkspace={importWorkspace}
            onReload={() => void loadPrivacyPreferences()}
            onSavePreferences={savePrivacyPreferences}
            onShowExportManifest={showExportManifest}
            onToggleExclusion={changeExclusionState}
            preferences={privacyPreferences}
          />
        )}
      </main>

      {showCreateProject && (
        <ProjectDialog
          busy={isCreatingProject}
          onClose={() => setShowCreateProject(false)}
          onSubmit={createProject}
        />
      )}
      {showArchiveDialog && selectedProject && (
        <ArchiveDialog
          busy={isArchivingProject}
          onClose={() => setShowArchiveDialog(false)}
          onConfirm={archiveSelectedProject}
          project={selectedProject}
        />
      )}
    </div>
  );
}

function TodayView({
  activity,
  continuity,
  onOpenProject,
  onSelectProject,
  privacyMode,
  projectOptions,
  selectedProject,
  selectionBusy,
}: {
  activity: WorkspaceSignal[];
  continuity: { label: string; value: string }[];
  onOpenProject: () => void;
  onSelectProject: (projectId: string) => Promise<void>;
  privacyMode: PrivacyMode;
  projectOptions: ProjectListItem[];
  selectedProject: Project | null;
  selectionBusy: boolean;
}) {
  if (!selectedProject) {
    return (
      <section className="first-use" aria-labelledby="first-use-heading">
        <p className="section-kicker">YOUR CONTINUITY DESK</p>
        <h2 id="first-use-heading">Begin with work worth returning to.</h2>
        <p>
          Create a project for the work you want to resume without reconstructing context. Aura
          stores this local release on this device and does not capture anything in the background.
        </p>
        <button className="primary-action" onClick={onOpenProject} type="button">
          Open projects <Icon name="arrow" />
        </button>
      </section>
    );
  }

  return (
    <div className="desk-grid">
      <section className="continuity-column" aria-labelledby="continuity-heading">
        <div className="project-context-row">
          <span className="section-kicker">CURRENT PROJECT</span>
          <label className="project-switcher-label" htmlFor="today-project-switcher">
            <span className="sr-only">Selected project</span>
            <select
              disabled={selectionBusy}
              id="today-project-switcher"
              onChange={(event) => void onSelectProject(event.target.value)}
              value={selectedProject.id}
            >
              {projectOptions.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </label>
        </div>

        <article className="continuity-sheet">
          <div className="sheet-project-heading">
            <div className="project-monogram" aria-hidden="true">
              {selectedProject.name.slice(0, 1)}
            </div>
            <div>
              <p>Project</p>
              <h2 id="continuity-heading">{selectedProject.name}</h2>
              <span>
                {selectedProject.status === "active"
                  ? "Active local project"
                  : "Paused local project"}
              </span>
            </div>
            <button className="outline-action" onClick={onOpenProject} type="button">
              Open project <Icon name="arrow" />
            </button>
          </div>
          <dl className="continuity-list">
            {continuity.map((entry) => (
              <div key={entry.label}>
                <dt>{entry.label}</dt>
                <dd className={entry.value.startsWith("No ") ? "is-empty" : ""}>{entry.value}</dd>
              </div>
            ))}
          </dl>
          <div className="sheet-footer">
            <span>
              <Icon name="shield" /> {privacyCopy(privacyMode)}
            </span>
            <time dateTime={selectedProject.updatedAt}>
              Last local change {formatTime(selectedProject.updatedAt)}
            </time>
          </div>
        </article>
      </section>

      <ActivityRail activity={activity} projectName={selectedProject.name} />
    </div>
  );
}

function ProjectsView({
  isSaving,
  onArchive,
  onCreate,
  onSave,
  onSelectProject,
  projects,
  selectedProject,
  selectionBusy,
}: {
  isSaving: boolean;
  onArchive: () => void;
  onCreate: () => void;
  onSave: (projectId: string, draft: ProjectDraft) => Promise<void>;
  onSelectProject: (projectId: string) => Promise<void>;
  projects: ProjectListItem[];
  selectedProject: Project | null;
  selectionBusy: boolean;
}) {
  if (!selectedProject) {
    return (
      <section className="first-use project-first-use" aria-labelledby="projects-empty-heading">
        <p className="section-kicker">LOCAL PROJECTS</p>
        <h2 id="projects-empty-heading">
          Start with a project you want to resume without re-explaining.
        </h2>
        <p>
          Aura does not invent a project history. Create the first local record when you are ready.
        </p>
        <button className="primary-action" onClick={onCreate} type="button">
          <Icon name="plus" /> Create project
        </button>
      </section>
    );
  }

  return (
    <div className="projects-layout">
      <section className="project-catalogue" aria-labelledby="project-list-heading">
        <div className="catalogue-header">
          <div>
            <p className="section-kicker">LOCAL PROJECTS</p>
            <h2 id="project-list-heading">In progress</h2>
          </div>
          <span>{projects.length}</span>
        </div>
        <div className="project-catalogue-list" aria-busy={selectionBusy}>
          {projects.map((project) => (
            <button
              aria-pressed={project.isSelected}
              className={`catalogue-row ${project.isSelected ? "is-selected" : ""}`}
              key={project.id}
              onClick={() => void onSelectProject(project.id)}
              type="button"
            >
              <span className="catalogue-initial" aria-hidden="true">
                {project.name.slice(0, 1)}
              </span>
              <span className="catalogue-summary">
                <strong>{project.name}</strong>
                <small>{project.nextStep || "No next step recorded"}</small>
              </span>
              <span className="catalogue-meta">
                <small>{project.status}</small>
                <Icon name="chevron" />
              </span>
            </button>
          ))}
        </div>
        <button className="catalogue-new" onClick={onCreate} type="button">
          <Icon name="plus" /> Add another project
        </button>
      </section>

      <ProjectEditor
        key={selectedProject.id}
        isSaving={isSaving}
        onArchive={onArchive}
        onSave={onSave}
        project={selectedProject}
      />
    </div>
  );
}

function ProjectEditor({
  isSaving,
  onArchive,
  onSave,
  project,
}: {
  isSaving: boolean;
  onArchive: () => void;
  onSave: (projectId: string, draft: ProjectDraft) => Promise<void>;
  project: Project;
}) {
  const [draft, setDraft] = useState<ProjectDraft>({
    name: project.name,
    goal: valueOrEmpty(project.goal),
    currentTask: valueOrEmpty(project.currentTask),
    blocker: valueOrEmpty(project.blocker),
    nextStep: valueOrEmpty(project.nextStep),
  });
  const [error, setError] = useState("");

  function updateDraft(field: keyof ProjectDraft, value: string) {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.name.trim()) {
      setError("Project name is required before Aura can save it locally.");
      return;
    }

    setError("");
    try {
      await onSave(project.id, draft);
    } catch {
      setError("Aura could not confirm this save. Review your fields and try again.");
    }
  }

  return (
    <section className="project-editor" aria-labelledby="editor-heading">
      <div className="editor-heading">
        <div>
          <p className="section-kicker">PROJECT DETAIL</p>
          <h2 id="editor-heading">Keep the thread visible.</h2>
        </div>
        <span className="project-status">
          <span aria-hidden="true" />
          {project.status === "active" ? "Active" : "Paused"}
        </span>
      </div>
      <form onSubmit={submit}>
        <div className="field-grid">
          <label className="field field-wide">
            <span>Project name</span>
            <input
              maxLength={120}
              onChange={(event) => updateDraft("name", event.target.value)}
              value={draft.name}
            />
          </label>
          <label className="field field-wide">
            <span>Goal</span>
            <textarea
              onChange={(event) => updateDraft("goal", event.target.value)}
              placeholder="What are you trying to make true?"
              rows={2}
              value={draft.goal}
            />
          </label>
          <label className="field">
            <span>Current task</span>
            <textarea
              onChange={(event) => updateDraft("currentTask", event.target.value)}
              placeholder="What are you working through now?"
              rows={3}
              value={draft.currentTask}
            />
          </label>
          <label className="field">
            <span>Next step</span>
            <textarea
              onChange={(event) => updateDraft("nextStep", event.target.value)}
              placeholder="What is the next deliberate action?"
              rows={3}
              value={draft.nextStep}
            />
          </label>
          <label className="field field-wide">
            <span>Blocker</span>
            <textarea
              onChange={(event) => updateDraft("blocker", event.target.value)}
              placeholder="What is currently in the way?"
              rows={2}
              value={draft.blocker}
            />
          </label>
        </div>
        {error && (
          <p className="field-error" role="alert">
            {error}
          </p>
        )}
        <div className="editor-actions">
          <p>
            <Icon name="shield" /> Stored in Aura’s local workspace.
          </p>
          <button className="primary-action" disabled={isSaving} type="submit">
            {isSaving ? "Saving locally…" : "Save changes"}
          </button>
        </div>
      </form>
      <div className="archive-zone">
        <div>
          <h3>Archive this project</h3>
          <p>Archive hides it from default views. Its local history remains retained.</p>
        </div>
        <button className="danger-action" onClick={onArchive} type="button">
          <Icon name="archive" /> Archive project
        </button>
      </div>
    </section>
  );
}

function CaptureView({
  defaultRetention,
  isSaving,
  onCancel,
  onSave,
  onSelectProject,
  privacyMode,
  projectOptions,
  selectedProject,
  selectionBusy,
}: {
  defaultRetention: CaptureRetention;
  isSaving: boolean;
  onCancel: () => void;
  onSave: (draft: CaptureDraft) => Promise<boolean>;
  onSelectProject: (projectId: string) => Promise<void>;
  privacyMode: PrivacyMode;
  projectOptions: ProjectListItem[];
  selectedProject: Project | null;
  selectionBusy: boolean;
}) {
  const [draft, setDraft] = useState<CaptureDraft>({
    projectId: selectedProject?.id || projectOptions[0]?.id || "",
    kind: "manual_note",
    label: "",
    content: "",
    classification: "standard",
    retention: undefined,
  });
  const [step, setStep] = useState<"edit" | "review">("edit");
  const [error, setError] = useState("");
  const isPaused = privacyMode === "paused";

  function updateDraft<Field extends keyof CaptureDraft>(field: Field, value: CaptureDraft[Field]) {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  function chooseProject(projectId: string) {
    updateDraft("projectId", projectId);
    void onSelectProject(projectId);
  }

  function beginReview(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (isPaused) {
      setError("Manual capture is paused. Resume it before saving anything locally.");
      return;
    }
    if (!draft.projectId) {
      setError("Choose a local project before reviewing this capture.");
      return;
    }
    if (!draft.label.trim() || !draft.content.trim()) {
      setError("A label and the exact content are required before Aura can show the save review.");
      return;
    }
    if (draft.kind === "url" && !/^https?:\/\//i.test(draft.content.trim())) {
      setError("A URL capture must begin with http:// or https://.");
      return;
    }
    setError("");
    setStep("review");
  }

  async function confirmSave() {
    const saved = await onSave(draft);
    if (saved) {
      setDraft((current) => ({ ...current, label: "", content: "" }));
      setStep("edit");
    }
  }

  const projectName =
    projectOptions.find((project) => project.id === draft.projectId)?.name || "No project selected";
  const contentLabel = draft.kind === "url" ? "URL" : "Content";

  if (projectOptions.length === 0) {
    return (
      <section className="first-use" aria-labelledby="capture-empty-heading">
        <p className="section-kicker">EXPLICIT LOCAL CAPTURE</p>
        <h2 id="capture-empty-heading">Choose a project before you preserve context.</h2>
        <p>
          Aura never files context into an unscoped global inbox. Create a local project first, then
          decide exactly what you want to save to it.
        </p>
        <button className="primary-action" onClick={onCancel} type="button">
          Open projects <Icon name="arrow" />
        </button>
      </section>
    );
  }

  return (
    <section className="capture-workspace" aria-labelledby="capture-heading">
      <div className="capture-intro">
        <div>
          <p className="section-kicker">EXPLICIT LOCAL CAPTURE</p>
          <h2 id="capture-heading">
            {step === "review" ? "Review before Aura keeps it." : "Add only what you mean to keep."}
          </h2>
        </div>
        <span className={`capture-state ${isPaused ? "is-paused" : ""}`}>
          <Icon name="shield" />{" "}
          {isPaused ? "Paused — saving is disabled" : "Manual only — no background capture"}
        </span>
      </div>

      {isPaused && (
        <div className="capture-warning" role="status">
          <strong>Manual capture is paused.</strong>
          Aura is not collecting, queueing, or saving anything from this form until you resume the
          local manual-only mode.
        </div>
      )}

      {step === "review" ? (
        <div
          className="capture-review"
          onKeyDown={(event) => event.key === "Escape" && setStep("edit")}
          tabIndex={-1}
        >
          <div className="review-heading">
            <div>
              <p className="section-kicker">SAVE REVIEW</p>
              <h3>Here is the exact local record Aura will create.</h3>
            </div>
            <button className="quiet-action" onClick={() => setStep("edit")} type="button">
              Edit capture
            </button>
          </div>
          <dl className="capture-review-grid">
            <div>
              <dt>Destination</dt>
              <dd>{projectName}</dd>
            </div>
            <div>
              <dt>Type</dt>
              <dd>{draft.kind.replace(/_/g, " ")}</dd>
            </div>
            <div>
              <dt>Classification</dt>
              <dd>{draft.classification}</dd>
            </div>
            <div>
              <dt>Retention</dt>
              <dd>{retentionLabel(draft.retention || defaultRetention)}</dd>
            </div>
          </dl>
          <article className="capture-preview">
            <span>Label</span>
            <h4>{draft.label}</h4>
            <span>{contentLabel}</span>
            <pre>{draft.content}</pre>
          </article>
          {draft.classification === "sensitive" && (
            <p className="sensitive-note">
              <strong>Sensitive is a label, not a guarantee.</strong> Edit or remove anything you do
              not want kept on this device before confirming.
            </p>
          )}
          <div className="capture-actions">
            <button className="quiet-action" onClick={onCancel} type="button">
              Cancel without saving
            </button>
            <button
              className="primary-action"
              disabled={isPaused || isSaving}
              onClick={() => void confirmSave()}
              type="button"
            >
              {isSaving ? "Saving locally…" : "Confirm and save locally"}
            </button>
          </div>
        </div>
      ) : (
        <form className="capture-form" onSubmit={beginReview}>
          <fieldset disabled={isPaused || isSaving}>
            <div className="capture-field-grid">
              <label className="field">
                <span>Destination project</span>
                <select
                  disabled={selectionBusy}
                  onChange={(event) => chooseProject(event.target.value)}
                  value={draft.projectId}
                >
                  {projectOptions.map((project) => (
                    <option key={project.id} value={project.id}>
                      {project.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>Capture type</span>
                <select
                  onChange={(event) => updateDraft("kind", event.target.value as CaptureKind)}
                  value={draft.kind}
                >
                  <option value="manual_note">Manual note</option>
                  <option value="pasted_text">Pasted text</option>
                  <option value="url">URL</option>
                </select>
              </label>
              <label className="field field-wide">
                <span>Label</span>
                <input
                  maxLength={120}
                  onChange={(event) => updateDraft("label", event.target.value)}
                  placeholder="What will help you recognize this later?"
                  value={draft.label}
                />
              </label>
              <label className="field field-wide">
                <span>{contentLabel}</span>
                <textarea
                  maxLength={20000}
                  onChange={(event) => updateDraft("content", event.target.value)}
                  placeholder={
                    draft.kind === "url"
                      ? "https://example.com/reference"
                      : "Paste or write only the context you want Aura to keep."
                  }
                  rows={8}
                  value={draft.content}
                />
                <small>
                  Aura does not read your clipboard or collect anything in the background.
                </small>
              </label>
              <label className="field">
                <span>Classification</span>
                <select
                  onChange={(event) =>
                    updateDraft("classification", event.target.value as CaptureClassification)
                  }
                  value={draft.classification}
                >
                  <option value="standard">Standard</option>
                  <option value="sensitive">Sensitive</option>
                </select>
              </label>
              <label className="field">
                <span>Retention</span>
                <select
                  onChange={(event) =>
                    updateDraft(
                      "retention",
                      event.target.value === "default"
                        ? undefined
                        : (event.target.value as CaptureRetention),
                    )
                  }
                  value={draft.retention || "default"}
                >
                  <option value="default">Use default ({retentionLabel(defaultRetention)})</option>
                  <option value="until_deleted">Until deleted</option>
                  <option value="review_in_30_days">Review in 30 days</option>
                </select>
              </label>
            </div>
          </fieldset>
          {draft.classification === "sensitive" && (
            <p className="sensitive-note">
              Sensitive data can include credentials, health, financial, or private client context.
              This label helps you review it; it does not automatically detect or remove anything.
            </p>
          )}
          {error && (
            <p className="field-error" role="alert">
              {error}
            </p>
          )}
          <div className="capture-actions">
            <button className="quiet-action" onClick={onCancel} type="button">
              Cancel without saving
            </button>
            <button className="primary-action" disabled={isPaused || isSaving} type="submit">
              Review before saving <Icon name="arrow" />
            </button>
          </div>
        </form>
      )}
    </section>
  );
}

function ActivityRail({
  activity,
  projectName,
}: {
  activity: WorkspaceSignal[];
  projectName: string;
}) {
  return (
    <aside className="activity-rail" aria-labelledby="record-heading">
      <div className="record-heading">
        <p className="section-kicker">LOCAL RECORD</p>
        <h2 id="record-heading">{projectName}</h2>
      </div>
      {activity.length === 0 ? (
        <div className="record-empty">
          <span className="empty-orbit" aria-hidden="true" />
          <h3>Nothing recorded yet.</h3>
          <p>
            When you make a deliberate local change or add a context marker, it will appear here for
            this project only.
          </p>
        </div>
      ) : (
        <ol className="record-list">
          {activity.map((signal) => (
            <li key={signal.id}>
              <span className={`record-dot is-${signal.kind}`} aria-hidden="true" />
              <div>
                <time dateTime={signal.time}>{formatTime(signal.time)}</time>
                <strong>{signal.title}</strong>
                <p>{signal.detail}</p>
              </div>
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}

function ProjectDialog({
  busy,
  onClose,
  onSubmit,
}: {
  busy: boolean;
  onClose: () => void;
  onSubmit: (draft: Pick<ProjectDraft, "name" | "goal" | "nextStep">) => Promise<void>;
}) {
  const [draft, setDraft] = useState({ name: "", goal: "", nextStep: "" });
  const [error, setError] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.name.trim()) {
      setError("Project name is required before Aura can save it locally.");
      return;
    }
    setError("");
    await onSubmit(draft);
  }

  return (
    <div className="dialog-backdrop" onMouseDown={onClose} role="presentation">
      <section
        aria-labelledby="new-project-heading"
        aria-modal="true"
        className="dialog-card"
        onKeyDown={(event) => event.key === "Escape" && onClose()}
        onMouseDown={(event) => event.stopPropagation()}
        role="dialog"
        tabIndex={-1}
      >
        <p className="section-kicker">NEW LOCAL PROJECT</p>
        <h2 id="new-project-heading">Start a thread you can return to.</h2>
        <p className="dialog-intro">
          Aura saves this local release on this device. Nothing is captured in the background.
        </p>
        <form onSubmit={submit}>
          <label className="field">
            <span>Project name</span>
            <input
              autoFocus
              maxLength={120}
              onChange={(event) =>
                setDraft((current) => ({ ...current, name: event.target.value }))
              }
              placeholder="e.g. Aura Desktop"
              value={draft.name}
            />
          </label>
          <label className="field">
            <span>
              Goal <em>Optional</em>
            </span>
            <textarea
              onChange={(event) =>
                setDraft((current) => ({ ...current, goal: event.target.value }))
              }
              placeholder="What do you want this project to achieve?"
              rows={2}
              value={draft.goal}
            />
          </label>
          <label className="field">
            <span>
              First next step <em>Optional</em>
            </span>
            <textarea
              onChange={(event) =>
                setDraft((current) => ({ ...current, nextStep: event.target.value }))
              }
              placeholder="What should Future You do first?"
              rows={2}
              value={draft.nextStep}
            />
          </label>
          {error && (
            <p className="field-error" role="alert">
              {error}
            </p>
          )}
          <div className="dialog-actions">
            <button className="secondary-action" disabled={busy} onClick={onClose} type="button">
              Cancel
            </button>
            <button className="primary-action" disabled={busy} type="submit">
              {busy ? "Creating locally…" : "Create project"}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function ArchiveDialog({
  busy,
  onClose,
  onConfirm,
  project,
}: {
  busy: boolean;
  onClose: () => void;
  onConfirm: () => Promise<void>;
  project: Project;
}) {
  return (
    <div className="dialog-backdrop" onMouseDown={onClose} role="presentation">
      <section
        aria-describedby="archive-copy"
        aria-labelledby="archive-heading"
        aria-modal="true"
        className="dialog-card archive-dialog"
        onKeyDown={(event) => event.key === "Escape" && onClose()}
        onMouseDown={(event) => event.stopPropagation()}
        role="dialog"
        tabIndex={-1}
      >
        <p className="section-kicker">ARCHIVE PROJECT</p>
        <h2 id="archive-heading">Archive {project.name}?</h2>
        <p className="dialog-intro" id="archive-copy">
          Archiving hides this project from default views. It does not erase its local history.
        </p>
        <div className="dialog-actions">
          <button className="secondary-action" disabled={busy} onClick={onClose} type="button">
            Cancel
          </button>
          <button
            className="danger-action"
            disabled={busy}
            onClick={() => void onConfirm()}
            type="button"
          >
            {busy ? "Archiving…" : "Archive project"}
          </button>
        </div>
      </section>
    </div>
  );
}

function LoadingDesk() {
  return (
    <div className="loading-desk" aria-label="Loading local workspace state">
      <div className="loading-heading" />
      <div className="loading-sheet" />
      <div className="loading-rail" />
    </div>
  );
}

export default App;

function MemoryView({
  decisions,
  error,
  isLoading,
  isSaving,
  onSave,
  project,
  projectOptions,
}: {
  decisions: DecisionClaim[];
  error: string;
  isLoading: boolean;
  isSaving: boolean;
  onSave: (draft: DecisionDraft, correctedDecisionId?: string) => Promise<boolean>;
  project: Project | null;
  projectOptions: ProjectListItem[];
}) {
  const [correcting, setCorrecting] = useState<DecisionClaim | null>(null);
  const [draft, setDraft] = useState<DecisionDraft>({
    projectId: project?.id || projectOptions[0]?.id || "",
    title: "",
    rationale: "",
    confidence: "medium",
    sourceLabels: "",
  });
  const [formError, setFormError] = useState("");

  function beginCorrection(decision: DecisionClaim) {
    setCorrecting(decision);
    setDraft({
      projectId: decision.projectId,
      title: decision.title,
      rationale: decision.rationale,
      confidence: decision.confidence,
      sourceLabels: decision.sources.map((source) => source.label).join("\n"),
    });
    setFormError("");
  }

  function cancelEditing() {
    setCorrecting(null);
    setDraft({
      projectId: project?.id || "",
      title: "",
      rationale: "",
      confidence: "medium",
      sourceLabels: "",
    });
    setFormError("");
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.title.trim() || !draft.rationale.trim() || !draft.sourceLabels.trim()) {
      setFormError("Decision, rationale, and at least one source or basis are required.");
      return;
    }
    setFormError("");
    const saved = await onSave(draft, correcting?.id);
    if (saved) {
      cancelEditing();
    }
  }

  if (!project) {
    return (
      <section className="first-use" aria-labelledby="memory-empty-heading">
        <p className="section-kicker">LOCAL MEMORY</p>
        <h2 id="memory-empty-heading">A decision belongs to a project.</h2>
        <p>
          Create a local project before recording a decision, its rationale, or its supporting
          basis.
        </p>
      </section>
    );
  }

  return (
    <div className="memory-layout">
      <section className="memory-form-sheet" aria-labelledby="memory-form-heading">
        <p className="section-kicker">{correcting ? "CORRECT A DECISION" : "RECORD A DECISION"}</p>
        <h2 id="memory-form-heading">
          {correcting ? "Preserve the change." : "Keep the reason with the choice."}
        </h2>
        <p className="memory-intro">
          {correcting
            ? "Aura creates a new local version and marks the earlier record as superseded. Nothing is silently overwritten."
            : `This decision is stored only in ${project.name}. Aura records it as your input, not an AI conclusion.`}
        </p>
        <form onSubmit={submit}>
          <label className="field">
            <span>Decision</span>
            <input
              maxLength={160}
              onChange={(event) =>
                setDraft((current) => ({ ...current, title: event.target.value }))
              }
              placeholder="What did you decide?"
              value={draft.title}
            />
          </label>
          <label className="field">
            <span>Rationale</span>
            <textarea
              maxLength={4000}
              onChange={(event) =>
                setDraft((current) => ({ ...current, rationale: event.target.value }))
              }
              placeholder="Why is this the right decision for now?"
              rows={4}
              value={draft.rationale}
            />
          </label>
          <div className="field-grid memory-fields">
            <label className="field">
              <span>Confidence</span>
              <select
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    confidence: event.target.value as DecisionDraft["confidence"],
                  }))
                }
                value={draft.confidence}
              >
                <option value="low">Low — needs more validation</option>
                <option value="medium">Medium — reasonable current basis</option>
                <option value="high">High — strong current basis</option>
              </select>
            </label>
            <label className="field">
              <span>Project</span>
              <input disabled value={project.name} />
            </label>
          </div>
          <label className="field">
            <span>Source or basis</span>
            <textarea
              aria-describedby="decision-source-help"
              maxLength={3000}
              onChange={(event) =>
                setDraft((current) => ({ ...current, sourceLabels: event.target.value }))
              }
              placeholder={
                "One reference per line\nExample: ADR-003\nExample: Team conversation, 13 Aug"
              }
              rows={4}
              value={draft.sourceLabels}
            />
            <small id="decision-source-help">
              Add the references, observations, or conversations that informed this choice.
            </small>
          </label>
          {(formError || error) && (
            <p className="field-error" role="alert">
              {formError || error}
            </p>
          )}
          <div className="editor-actions">
            <p>
              <Icon name="shield" /> User-authored and stored in this local project.
            </p>
            <div className="inline-actions">
              {correcting && (
                <button className="outline-action" onClick={cancelEditing} type="button">
                  Cancel
                </button>
              )}
              <button className="primary-action" disabled={isSaving} type="submit">
                {isSaving ? "Saving locally…" : correcting ? "Save correction" : "Save decision"}
              </button>
            </div>
          </div>
        </form>
      </section>

      <section className="decision-ledger" aria-labelledby="decision-ledger-heading">
        <div className="ledger-heading">
          <div>
            <p className="section-kicker">{project.name.toUpperCase()}</p>
            <h2 id="decision-ledger-heading">Decision ledger</h2>
          </div>
          <span>{decisions.length}</span>
        </div>
        {isLoading ? (
          <p className="ledger-empty">Loading this project’s local decision record…</p>
        ) : decisions.length === 0 ? (
          <p className="ledger-empty">No decisions are recorded for this project yet.</p>
        ) : (
          <div className="decision-list">
            {decisions.map((decision) => (
              <article
                className={`decision-card ${decision.status === "superseded" ? "is-superseded" : ""}`}
                key={decision.id}
              >
                <div className="decision-card-topline">
                  <span className={`confidence-chip confidence-${decision.confidence}`}>
                    {decision.confidence} confidence
                  </span>
                  <time dateTime={decision.createdAt}>{formatTime(decision.createdAt)}</time>
                </div>
                <h3>{decision.title}</h3>
                <p>{decision.rationale}</p>
                <ul className="source-list" aria-label="Decision sources">
                  {decision.sources.map((source) => (
                    <li key={source.id}>{source.label}</li>
                  ))}
                </ul>
                {decision.status === "superseded" ? (
                  <p className="decision-state">Superseded by a newer local decision.</p>
                ) : (
                  <button
                    className="text-link"
                    onClick={() => beginCorrection(decision)}
                    type="button"
                  >
                    Correct this decision
                  </button>
                )}
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function SettingsView({
  error,
  exportError,
  exportBusy,
  exportManifest,
  exportNotice,
  isLoading,
  isSaving,
  onAddExclusion,
  onExportWorkspace,
  onImportWorkspace,
  onReload,
  onSavePreferences,
  onShowExportManifest,
  onToggleExclusion,
  preferences,
}: {
  error: string;
  exportError: string;
  exportBusy: boolean;
  exportManifest: ExportManifest | null;
  exportNotice: string;
  isLoading: boolean;
  isSaving: boolean;
  onAddExclusion: (kind: ExclusionKind, value: string) => Promise<boolean>;
  onExportWorkspace: () => Promise<void>;
  onImportWorkspace: () => Promise<void>;
  onReload: () => void;
  onSavePreferences: (
    privacyMode: PrivacyMode,
    defaultCaptureRetention: CaptureRetention,
  ) => Promise<boolean>;
  onToggleExclusion: (exclusionId: string, isEnabled: boolean) => Promise<void>;
  onShowExportManifest: () => Promise<void>;
  preferences: PrivacyPreferences | null;
}) {
  const [keyVaultStatus, setKeyVaultStatus] = useState<KeyVaultStatus | null>(null);

  const loadKeyVaultStatus = useCallback(async () => {
    try {
      const status = await invoke<KeyVaultStatus>("key_vault_status");
      setKeyVaultStatus(status);
    } catch {
      setKeyVaultStatus(null);
    }
  }, []);
  if (isLoading && !preferences) {
    return <LoadingDesk />;
  }

  if (!preferences) {
    return (
      <section className="settings-workspace" aria-labelledby="settings-heading">
        <p className="section-kicker">LOCAL PRIVACY CONTROLS</p>
        <h2 id="settings-heading">Preferences are unavailable right now.</h2>
        <p>
          Aura has not changed any saved capture, project, or decision. Retry only reloads the local
          preferences stored on this device.
        </p>
        {error && <p className="field-error">{error}</p>}
        <button className="primary-action" onClick={onReload} type="button">
          Try again <Icon name="arrow" />
        </button>
      </section>
    );
  }

  return (
    <SettingsForm
      key={`${preferences.privacyMode}-${preferences.defaultCaptureRetention}-${preferences.exclusions.length}`}
      error={error}
      exportError={exportError}
      exportBusy={exportBusy}
      exportManifest={exportManifest}
      exportNotice={exportNotice}
      isSaving={isSaving}
      onAddExclusion={onAddExclusion}
      onExportWorkspace={onExportWorkspace}
      onImportWorkspace={onImportWorkspace}
      onSavePreferences={onSavePreferences}
      keyVaultStatus={keyVaultStatus}
      loadKeyVaultStatus={loadKeyVaultStatus}
      onToggleExclusion={onToggleExclusion}
      onShowExportManifest={onShowExportManifest}
      preferences={preferences}
    />
  );
}

function SettingsForm({
  error,
  exportError,
  exportBusy,
  exportManifest,
  exportNotice,
  isSaving,
  keyVaultStatus,
  loadKeyVaultStatus,
  onAddExclusion,
  onExportWorkspace,
  onImportWorkspace,
  onSavePreferences,
  onToggleExclusion,
  onShowExportManifest,
  preferences,
}: {
  error: string;
  exportError: string;
  exportBusy: boolean;
  exportManifest: ExportManifest | null;
  exportNotice: string;
  isSaving: boolean;
  keyVaultStatus: KeyVaultStatus | null;
  loadKeyVaultStatus: () => void;
  onAddExclusion: (kind: ExclusionKind, value: string) => Promise<boolean>;
  onExportWorkspace: () => Promise<void>;
  onImportWorkspace: () => Promise<void>;
  onSavePreferences: (
    privacyMode: PrivacyMode,
    defaultCaptureRetention: CaptureRetention,
  ) => Promise<boolean>;
  onToggleExclusion: (exclusionId: string, isEnabled: boolean) => Promise<void>;
  onShowExportManifest: () => Promise<void>;
  preferences: PrivacyPreferences;
}) {
  const [privacyMode, setPrivacyMode] = useState<PrivacyMode>(preferences.privacyMode);
  const [defaultRetention, setDefaultRetention] = useState<CaptureRetention>(
    preferences.defaultCaptureRetention,
  );
  const [exclusionKind, setExclusionKind] = useState<ExclusionKind>("application");
  const [exclusionValue, setExclusionValue] = useState("");
  const [exclusionError, setExclusionError] = useState("");

  async function submitPreferences(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await onSavePreferences(privacyMode, defaultRetention);
  }

  async function submitExclusion(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedValue = exclusionValue.trim();
    if (!trimmedValue) {
      setExclusionError("Enter the application, domain, or project name to exclude.");
      return;
    }
    setExclusionError("");
    const created = await onAddExclusion(exclusionKind, trimmedValue);
    if (created) {
      setExclusionValue("");
    }
  }

  return (
    <section className="settings-workspace" aria-labelledby="settings-heading">
      <div className="settings-intro">
        <div>
          <p className="section-kicker">LOCAL PRIVACY CONTROLS</p>
          <h2 id="settings-heading">You set the boundary.</h2>
          <p>
            These settings are stored only in Aura’s local workspace. They never enable screenshots,
            clipboard reading, microphone access, background collection, or network sync.
          </p>
        </div>
        <span className="local-status">
          <span aria-hidden="true" /> Local only
        </span>
      </div>

      <div className="settings-grid">
        <form className="settings-card" onSubmit={submitPreferences}>
          <div className="settings-card-heading">
            <div>
              <p className="section-kicker">CAPTURE BEHAVIOR</p>
              <h3>Manual capture state</h3>
            </div>
            <Icon name="shield" />
          </div>
          <fieldset disabled={isSaving}>
            <label className="setting-choice">
              <input
                checked={privacyMode === "manual_only"}
                name="privacy-mode"
                onChange={() => setPrivacyMode("manual_only")}
                type="radio"
                value="manual_only"
              />
              <span>
                <strong>Manual only</strong>
                <small>
                  Save only a note, pasted text, or URL that you deliberately review and confirm.
                </small>
              </span>
            </label>
            <label className="setting-choice">
              <input
                checked={privacyMode === "paused"}
                name="privacy-mode"
                onChange={() => setPrivacyMode("paused")}
                type="radio"
                value="paused"
              />
              <span>
                <strong>Paused</strong>
                <small>
                  Block context markers and manual capture until you explicitly resume manual-only
                  mode.
                </small>
              </span>
            </label>
            <label className="field settings-field">
              <span>Default capture retention</span>
              <select
                aria-label="Default capture retention"
                onChange={(event) => setDefaultRetention(event.target.value as CaptureRetention)}
                value={defaultRetention}
              >
                <option value="until_deleted">Until deleted</option>
                <option value="review_in_30_days">Review in 30 days</option>
              </select>
              <small>
                Capture can still choose an explicit retention value. If it does not, Aura resolves
                this stored local default in Rust.
              </small>
            </label>
          </fieldset>
          <button className="primary-action" disabled={isSaving} type="submit">
            {isSaving ? "Saving locally…" : "Save privacy preferences"}
          </button>
          {error && (
            <p className="field-error" role="alert">
              {error}
            </p>
          )}
        </form>

        <section className="settings-card exclusion-card" aria-labelledby="exclusions-heading">
          <div className="settings-card-heading">
            <div>
              <p className="section-kicker">FUTURE-READY BOUNDARY</p>
              <h3 id="exclusions-heading">Exclusion rules</h3>
            </div>
            <Icon name="lock" />
          </div>
          <p className="settings-note">
            Aura V0 has no passive observation adapter. These rules are saved now as an explicit
            product boundary for any later, separately approved local feature; they do not monitor
            or block another application today.
          </p>
          <form className="exclusion-form" onSubmit={submitExclusion}>
            <label className="field">
              <span>Rule type</span>
              <select
                aria-label="Exclusion rule type"
                onChange={(event) => setExclusionKind(event.target.value as ExclusionKind)}
                value={exclusionKind}
              >
                <option value="application">Application</option>
                <option value="domain">Domain</option>
                <option value="project">Project</option>
              </select>
            </label>
            <label className="field exclusion-value-field">
              <span>Value</span>
              <input
                aria-label="Exclusion value"
                maxLength={160}
                onChange={(event) => setExclusionValue(event.target.value)}
                placeholder={exclusionKind === "domain" ? "example.com" : "Name to exclude"}
                value={exclusionValue}
              />
            </label>
            <button className="quiet-action" disabled={isSaving} type="submit">
              Add rule
            </button>
          </form>
          {exclusionError && (
            <p className="field-error" role="alert">
              {exclusionError}
            </p>
          )}
          {preferences.exclusions.length === 0 ? (
            <p className="empty-rule-state">No future exclusion rules saved.</p>
          ) : (
            <ul className="exclusion-list">
              {preferences.exclusions.map((rule) => (
                <li key={rule.id}>
                  <div>
                    <strong>{rule.value}</strong>
                    <span>{rule.kind}</span>
                  </div>
                  <label className="toggle-label">
                    <span className="sr-only">Enable {rule.value} exclusion</span>
                    <input
                      checked={rule.isEnabled}
                      disabled={isSaving}
                      onChange={(event) => void onToggleExclusion(rule.id, event.target.checked)}
                      type="checkbox"
                    />
                    <span>{rule.isEnabled ? "Enabled" : "Disabled"}</span>
                  </label>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section className="settings-card" aria-labelledby="export-heading">
          <div className="settings-card-heading">
            <div>
              <p className="section-kicker">DATA OWNERSHIP</p>
              <h3 id="export-heading">Export &amp; recovery</h3>
            </div>
            <Icon name="lock" />
          </div>
          <p className="settings-note">
            Export writes your entire encrypted workspace, including the key-protecting envelope, to
            a single sealed archive you keep on your own device. The archive can only be opened by
            an Aura installation with the same workspace key, so losing that key means losing the
            archive. Nothing is ever sent to a network.
          </p>
          <div className="settings-actions">
            <button
              className="primary-action"
              disabled={exportBusy}
              onClick={() => void onExportWorkspace()}
              type="button"
            >
              Export workspace&hellip;
            </button>
            <button
              className="quiet-action"
              disabled={exportBusy}
              onClick={() => void onImportWorkspace()}
              type="button"
            >
              Restore from archive&hellip;
            </button>
            <button
              className="quiet-action"
              onClick={() => void onShowExportManifest()}
              type="button"
            >
              Preview export contents
            </button>
          </div>
          {exportNotice && (
            <p className="settings-note" role="status">
              {exportNotice}
            </p>
          )}
          {exportError && (
            <p className="field-error" role="alert">
              {exportError}
            </p>
          )}
          {exportManifest && (
            <dl className="export-manifest" aria-label="Export contents">
              <div>
                <dt>Format version</dt>
                <dd>{exportManifest.formatVersion}</dd>
              </div>
              <div>
                <dt>Exported at</dt>
                <dd>{new Date(exportManifest.exportedAt).toLocaleString()}</dd>
              </div>
              <div>
                <dt>Exported by version</dt>
                <dd>{exportManifest.exportedByVersion}</dd>
              </div>
              <div>
                <dt>Projects</dt>
                <dd>{exportManifest.recordCounts.projects}</dd>
              </div>
              <div>
                <dt>Captures</dt>
                <dd>{exportManifest.recordCounts.captures}</dd>
              </div>
              <div>
                <dt>Decisions</dt>
                <dd>{exportManifest.recordCounts.decisions}</dd>
              </div>
              <div>
                <dt>Exclusion rules</dt>
                <dd>{exportManifest.recordCounts.exclusionRules}</dd>
              </div>
              <div>
                <dt>Settings</dt>
                <dd>{exportManifest.recordCounts.settings}</dd>
              </div>
              <div>
                <dt>SHA-256 checksum</dt>
                <dd>{exportManifest.payloadChecksum.slice(0, 32)}&hellip;</dd>
              </div>
            </dl>
          )}
        </section>

        <div className="settings-section">
          <h3 className="settings-card-heading">Local key protection</h3>
          <p className="settings-note">
            Aura wraps its local data-encryption key with the Windows user boundary (DPAPI) and
            encrypts values with an authenticated envelope. Only a status summary is exposed; raw
            key material never leaves this device and is never shown here.
          </p>
          <div className="preference-row">
            <span>Wrapped key stored</span>
            <span>
              {keyVaultStatus ? (keyVaultStatus.wrappedKeyPersisted ? "Yes" : "No") : "—"}
            </span>
          </div>
          <button className="quiet-action" type="button" onClick={() => void loadKeyVaultStatus()}>
            Check key protection
          </button>
          {keyVaultStatus && (
            <p className="settings-note">
              Key length: {keyVaultStatus.keyLength} bytes · Sealed format version{" "}
              {keyVaultStatus.sealedVersion}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}
