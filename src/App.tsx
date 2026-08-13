import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";

type PrivacyMode = "focused" | "paused";
type SignalKind = "context" | "decision" | "memory" | "system" | "project";

type Project = {
  id: string;
  name: string;
  status: string;
  signal: string;
  updatedAt: string;
};

type Signal = {
  id: string;
  kind: SignalKind;
  title: string;
  detail: string;
  time: string;
};

type WorkspaceSnapshot = {
  activeProject: string;
  continuityNote: string;
  nextStep: string;
  privacyMode: PrivacyMode;
  projects: Project[];
  signals: Signal[];
};

const emptySnapshot: WorkspaceSnapshot = {
  activeProject: "No local project selected",
  continuityNote:
    "Aura is waiting for its local workspace to load. No desktop capture, cloud sync, or AI provider is enabled.",
  nextStep: "Create a local project to begin an intentional continuity record.",
  privacyMode: "focused",
  projects: [],
  signals: [],
};

const navigation = ["Now", "Projects", "Memory", "Cortex", "Controls"];

function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot>(emptySnapshot);
  const [activeView, setActiveView] = useState("Now");
  const [isLoading, setIsLoading] = useState(true);
  const [isCapturing, setIsCapturing] = useState(false);
  const [isSavingPrivacy, setIsSavingPrivacy] = useState(false);
  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [showProjectForm, setShowProjectForm] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [notice, setNotice] = useState("");
  const [loadError, setLoadError] = useState("");

  const dismissNotice = useCallback(() => {
    window.setTimeout(() => setNotice(""), 2600);
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
        "Aura could not open the local workspace. Your existing records were not changed. Try again, or restart Aura to recover the local database connection.",
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

  const activeProject = useMemo(
    () =>
      snapshot.projects.find((project) => project.name === snapshot.activeProject) ??
      snapshot.projects[0],
    [snapshot],
  );

  async function togglePrivacyMode() {
    const nextMode: PrivacyMode = snapshot.privacyMode === "focused" ? "paused" : "focused";
    setIsSavingPrivacy(true);

    try {
      await invoke("set_privacy_mode", { mode: nextMode });
      setSnapshot((current) => ({ ...current, privacyMode: nextMode }));
      setNotice(nextMode === "focused" ? "Intentional capture ready" : "All capture paused");
    } catch {
      setNotice("Aura could not update the local privacy setting");
    } finally {
      setIsSavingPrivacy(false);
      dismissNotice();
    }
  }

  async function captureContext() {
    if (snapshot.privacyMode === "paused") {
      setNotice("Resume intentional capture before adding context");
      dismissNotice();
      return;
    }

    if (!activeProject) {
      setNotice("Create or select a local project before adding context");
      dismissNotice();
      return;
    }

    setIsCapturing(true);
    try {
      await invoke("record_intentional_capture", { projectId: activeProject.id });
      await loadWorkspace();
      setNotice("Context marker saved locally");
    } catch {
      setNotice("Aura could not save the local context marker");
    } finally {
      setIsCapturing(false);
      dismissNotice();
    }
  }

  async function createProject(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = projectName.trim();
    if (!name) {
      setNotice("Enter a project name before saving");
      dismissNotice();
      return;
    }

    setIsCreatingProject(true);
    try {
      const project = await invoke<Project>("create_project", { input: { name } });
      setProjectName("");
      setShowProjectForm(false);
      await loadWorkspace();
      setSnapshot((current) => ({ ...current, activeProject: project.name }));
      setNotice("Project saved locally");
    } catch {
      setNotice("Aura could not save the local project");
    } finally {
      setIsCreatingProject(false);
      dismissNotice();
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Aura navigation">
        <div className="brand-lockup">
          <div className="brand-mark" aria-hidden="true">
            A
          </div>
          <div>
            <p className="eyebrow">PERSONAL OPERATING SYSTEM</p>
            <h1>Aura</h1>
          </div>
        </div>

        <nav className="main-nav" aria-label="Main views">
          {navigation.map((item) => (
            <button
              className={`nav-item ${activeView === item ? "active" : ""}`}
              key={item}
              onClick={() => setActiveView(item)}
              type="button"
            >
              <span className="nav-indicator" aria-hidden="true" />
              {item}
            </button>
          ))}
        </nav>

        <div className="sidebar-bottom">
          <div className="privacy-card">
            <div className="privacy-card-top">
              <span className={`status-dot ${snapshot.privacyMode}`} aria-hidden="true" />
              <span>
                {snapshot.privacyMode === "focused" ? "Intentional capture" : "Capture paused"}
              </span>
            </div>
            <p>
              {snapshot.privacyMode === "focused"
                ? "Aura only receives context you explicitly allow."
                : "No context is being collected or queued."}
            </p>
            <button
              className="text-button"
              disabled={isSavingPrivacy}
              onClick={togglePrivacyMode}
              type="button"
            >
              {isSavingPrivacy
                ? "Updating…"
                : snapshot.privacyMode === "focused"
                  ? "Pause capture"
                  : "Resume capture"}
            </button>
          </div>
          <p className="build-label">WINDOWS V0 · 0.1.0</p>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div>
            <p className="eyebrow">
              {activeView.toUpperCase()} / {snapshot.activeProject.toUpperCase()}
            </p>
            <h2>Good afternoon, Eternal.</h2>
          </div>
          <div className="topbar-actions">
            <span className="local-badge">Local-first</span>
            <button
              className="capture-button"
              disabled={isCapturing || !activeProject}
              onClick={captureContext}
              type="button"
            >
              {isCapturing ? "Saving context…" : "Add context"}
            </button>
          </div>
        </header>

        {notice && (
          <div className="notice" role="status">
            {notice}
          </div>
        )}

        {loadError && (
          <div className="notice error-notice" role="alert">
            <span>{loadError}</span>
            <button className="text-button" onClick={() => void loadWorkspace()} type="button">
              Try again
            </button>
          </div>
        )}

        <section className="brief-card" aria-labelledby="continuity-heading">
          <div className="brief-card-heading">
            <div>
              <p className="section-kicker">CONTINUITY BRIEF</p>
              <h3 id="continuity-heading">Pick up without reconstructing.</h3>
            </div>
            <span className="live-chip">LOCAL STATE</span>
          </div>
          <p className="continuity-copy">{snapshot.continuityNote}</p>
          <div className="next-step">
            <span>Next deliberate step</span>
            <strong>{snapshot.nextStep}</strong>
          </div>
        </section>

        <section className="dashboard-grid" aria-label="Aura workspace overview">
          <article className="panel project-panel">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">ACTIVE PROJECTS</p>
                <h3>Work that needs continuity</h3>
              </div>
              <button
                className="panel-action"
                onClick={() => setShowProjectForm((current) => !current)}
                type="button"
              >
                {showProjectForm ? "Cancel" : "New project"}
              </button>
            </div>

            {showProjectForm && (
              <form className="project-create-form" onSubmit={createProject}>
                <label htmlFor="project-name">Project name</label>
                <div className="project-create-controls">
                  <input
                    autoFocus
                    id="project-name"
                    maxLength={120}
                    onChange={(event) => setProjectName(event.target.value)}
                    placeholder="e.g. Ascend"
                    value={projectName}
                  />
                  <button className="panel-action" disabled={isCreatingProject} type="submit">
                    {isCreatingProject ? "Saving…" : "Save locally"}
                  </button>
                </div>
              </form>
            )}

            <div className="project-list">
              {snapshot.projects.length === 0 ? (
                <p className="empty-state">No local projects yet. Create one to begin.</p>
              ) : (
                snapshot.projects.map((project) => (
                  <button
                    className="project-row"
                    key={project.id}
                    onClick={() =>
                      setSnapshot((current) => ({ ...current, activeProject: project.name }))
                    }
                    type="button"
                  >
                    <div className="project-name-block">
                      <span className="project-dot" aria-hidden="true" />
                      <div>
                        <strong>{project.name}</strong>
                        <span>{project.signal}</span>
                      </div>
                    </div>
                    <div className="project-progress-block">
                      <span>{project.status}</span>
                    </div>
                    <time>{project.updatedAt}</time>
                  </button>
                ))
              )}
            </div>
          </article>

          <article className="panel signal-panel">
            <div className="panel-heading">
              <div>
                <p className="section-kicker">MEMORY SIGNALS</p>
                <h3>What Aura is holding</h3>
              </div>
              <span className="signal-count">{snapshot.signals.length}</span>
            </div>
            {snapshot.signals.length === 0 ? (
              <p className="empty-state">No local activity has been recorded yet.</p>
            ) : (
              <ol className="signal-list">
                {snapshot.signals.map((signal) => (
                  <li className="signal-item" key={signal.id}>
                    <span className={`signal-icon ${signal.kind}`} aria-hidden="true">
                      {signal.kind.slice(0, 1).toUpperCase()}
                    </span>
                    <div>
                      <strong>{signal.title}</strong>
                      <p>{signal.detail}</p>
                      <time>{signal.time}</time>
                    </div>
                  </li>
                ))}
              </ol>
            )}
          </article>
        </section>

        <section className="foundation-row" aria-label="Aura architecture status">
          <div className="foundation-copy">
            <p className="section-kicker">V0 FOUNDATION</p>
            <h3>Designed to earn trust before it automates.</h3>
            <p>
              Windows system awareness, project memory, and AI actions stay behind explicit
              permissions and readable local records.
            </p>
          </div>
          <div className="foundation-pillars">
            <span>
              <b>01</b> Intentional perception
            </span>
            <span>
              <b>02</b> Local memory contract
            </span>
            <span>
              <b>03</b> Human-approved action
            </span>
          </div>
        </section>

        {isLoading && <div className="loading-line" aria-label="Loading local workspace state" />}
      </main>
    </div>
  );
}

export default App;
