import { invoke } from "@tauri-apps/api/core";
import { FormEvent, ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";

type PrivacyMode = "focused" | "paused";
type View = "today" | "projects";

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
  kind: "context" | "project" | "system";
  title: string;
  detail: string;
  time: string;
};

type WorkspaceSnapshot = {
  privacyMode: PrivacyMode;
  selectedProject: Project | null;
  projects: ProjectListItem[];
  activity: WorkspaceSignal[];
};

type ProjectDraft = {
  name: string;
  goal: string;
  currentTask: string;
  blocker: string;
  nextStep: string;
};

const emptySnapshot: WorkspaceSnapshot = {
  privacyMode: "focused",
  selectedProject: null,
  projects: [],
  activity: [],
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
  return mode === "focused"
    ? "Manual only — no background capture"
    : "Paused — no context is collected or queued";
}

function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot>(emptySnapshot);
  const [activeView, setActiveView] = useState<View>("today");
  const [isLoading, setIsLoading] = useState(true);
  const [isSavingPrivacy, setIsSavingPrivacy] = useState(false);
  const [isCapturing, setIsCapturing] = useState(false);
  const [isSelectingProject, setIsSelectingProject] = useState(false);
  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [isSavingProject, setIsSavingProject] = useState(false);
  const [isArchivingProject, setIsArchivingProject] = useState(false);
  const [showCreateProject, setShowCreateProject] = useState(false);
  const [showArchiveDialog, setShowArchiveDialog] = useState(false);
  const [notice, setNotice] = useState("");
  const [loadError, setLoadError] = useState("");

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

  const continuity = useMemo(
    () => [
      { label: "Goal", value: selectedProject?.goal || "No goal recorded" },
      { label: "Current task", value: selectedProject?.currentTask || "No current task recorded" },
      { label: "Next step", value: selectedProject?.nextStep || "No next step recorded" },
      { label: "Blocker", value: selectedProject?.blocker || "No blocker recorded" },
    ],
    [selectedProject],
  );

  async function togglePrivacyMode() {
    const nextMode: PrivacyMode = snapshot.privacyMode === "focused" ? "paused" : "focused";
    setIsSavingPrivacy(true);

    try {
      await invoke("set_privacy_mode", { mode: nextMode });
      setSnapshot((current) => ({ ...current, privacyMode: nextMode }));
      setNotice(nextMode === "focused" ? "Manual context is ready" : "Manual context is paused");
    } catch {
      setNotice("Aura could not update the local privacy setting");
    } finally {
      setIsSavingPrivacy(false);
      dismissNotice();
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

  async function captureContext() {
    if (snapshot.privacyMode === "paused") {
      setNotice("Resume manual context before adding a context marker");
      dismissNotice();
      return;
    }

    if (!selectedProject) {
      setNotice("Select a local project before adding a context marker");
      dismissNotice();
      return;
    }

    setIsCapturing(true);
    try {
      await invoke("record_intentional_capture", { projectId: selectedProject.id });
      await loadWorkspace();
      setNotice("Context marker saved locally");
    } catch {
      setNotice("Aura could not save the local context marker");
    } finally {
      setIsCapturing(false);
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

  const heading = activeView === "today" ? "Continue with clarity." : "Projects";
  const subheading =
    activeView === "today"
      ? "Resume one deliberate thread at a time."
      : "Local projects, held in context.";

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
            const isAvailable = item.id === "today" || item.id === "projects";
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
            <span>{snapshot.privacyMode === "focused" ? "Manual only" : "Paused"}</span>
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
              : snapshot.privacyMode === "focused"
                ? "Pause manual context"
                : "Resume manual context"}
          </button>
        </div>
        <p className="build-stamp">AURA DESKTOP · LOCAL V0</p>
      </aside>

      <main className="desk-main">
        <header className="desk-header">
          <div>
            <p className="route-eyebrow">{activeView === "today" ? "TODAY" : "PROJECTS"}</p>
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
            ) : (
              <button
                className="primary-action"
                disabled={isCapturing || !selectedProject || snapshot.privacyMode === "paused"}
                onClick={captureContext}
                type="button"
              >
                <Icon name="plus" />
                {isCapturing ? "Saving locally…" : "Add context marker"}
              </button>
            )}
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
            projectOptions={snapshot.projects}
            selectedProject={selectedProject}
            selectionBusy={isSelectingProject}
          />
        ) : (
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
  projectOptions,
  selectedProject,
  selectionBusy,
}: {
  activity: WorkspaceSignal[];
  continuity: { label: string; value: string }[];
  onOpenProject: () => void;
  onSelectProject: (projectId: string) => Promise<void>;
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
              <Icon name="shield" /> {privacyCopy("focused")}
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
