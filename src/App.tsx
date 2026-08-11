import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import "./App.css";

type PrivacyMode = "focused" | "paused";

type Project = {
  id: string;
  name: string;
  status: string;
  signal: string;
  progress: number;
  updatedAt: string;
};

type Signal = {
  id: string;
  kind: "context" | "decision" | "memory";
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

const fallbackSnapshot: WorkspaceSnapshot = {
  activeProject: "Aura Desktop",
  continuityNote:
    "The architecture scaffold is active. Capture remains intentional while the Windows perception contract is being verified.",
  nextStep: "Define the first user-authorized context capture flow.",
  privacyMode: "focused",
  projects: [
    {
      id: "aura",
      name: "Aura Desktop",
      status: "In progress",
      signal: "Architecture baseline",
      progress: 24,
      updatedAt: "Now",
    },
    {
      id: "ascend",
      name: "Ascend",
      status: "Paused",
      signal: "Awaiting scope review",
      progress: 58,
      updatedAt: "Yesterday",
    },
    {
      id: "eternal",
      name: "Eternal Studios",
      status: "Active",
      signal: "Brand-system decisions",
      progress: 72,
      updatedAt: "2 days ago",
    },
  ],
  signals: [
    {
      id: "signal-1",
      kind: "decision",
      title: "Windows-first stack selected",
      detail: "Tauri 2, React, TypeScript, and Rust establish the initial desktop boundary.",
      time: "Just now",
    },
    {
      id: "signal-2",
      kind: "context",
      title: "Intentional capture is active",
      detail: "Aura will not observe or send desktop context until an explicit capture workflow exists.",
      time: "Today",
    },
    {
      id: "signal-3",
      kind: "memory",
      title: "Research mandate linked",
      detail: "The saved master research mandate is the source of truth for product and technical decisions.",
      time: "Today",
    },
  ],
};

const navigation = ["Now", "Projects", "Memory", "Cortex", "Controls"];

function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot>(fallbackSnapshot);
  const [activeView, setActiveView] = useState("Now");
  const [isLoading, setIsLoading] = useState(true);
  const [isCapturing, setIsCapturing] = useState(false);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    async function loadWorkspace() {
      try {
        const loaded = await invoke<WorkspaceSnapshot>("get_workspace_snapshot");
        setSnapshot(loaded);
      } catch {
        // The visual shell is intentionally useful before the local service layer is complete.
        setSnapshot(fallbackSnapshot);
      } finally {
        setIsLoading(false);
      }
    }

    void loadWorkspace();
  }, []);

  const activeProject = useMemo(
    () => snapshot.projects.find((project) => project.name === snapshot.activeProject) ?? snapshot.projects[0],
    [snapshot],
  );

  async function togglePrivacyMode() {
    const nextMode: PrivacyMode = snapshot.privacyMode === "focused" ? "paused" : "focused";
    setSnapshot((current) => ({ ...current, privacyMode: nextMode }));

    try {
      await invoke("set_privacy_mode", { mode: nextMode });
    } catch {
      // Local optimistic state keeps the demonstration shell usable when the command layer is unavailable.
    }

    setNotice(nextMode === "focused" ? "Intentional capture ready" : "All capture paused");
    window.setTimeout(() => setNotice(""), 2600);
  }

  async function captureContext() {
    if (snapshot.privacyMode === "paused") {
      setNotice("Resume intentional capture before adding context");
      window.setTimeout(() => setNotice(""), 2600);
      return;
    }

    setIsCapturing(true);
    try {
      await invoke("record_intentional_capture", { projectId: activeProject?.id ?? "aura" });
      setNotice("Context marker saved locally");
    } catch {
      setNotice("Capture workflow is connected but not yet enabled on this device");
    } finally {
      window.setTimeout(() => {
        setIsCapturing(false);
        setNotice("");
      }, 1800);
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Aura navigation">
        <div className="brand-lockup">
          <div className="brand-mark" aria-hidden="true">A</div>
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
              <span>{snapshot.privacyMode === "focused" ? "Intentional capture" : "Capture paused"}</span>
            </div>
            <p>
              {snapshot.privacyMode === "focused"
                ? "Aura only receives context you explicitly allow."
                : "No context is being collected or queued."}
            </p>
            <button className="text-button" onClick={togglePrivacyMode} type="button">
              {snapshot.privacyMode === "focused" ? "Pause capture" : "Resume capture"}
            </button>
          </div>
          <p className="build-label">WINDOWS V0 · 0.1.0</p>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div>
            <p className="eyebrow">{activeView.toUpperCase()} / {snapshot.activeProject.toUpperCase()}</p>
            <h2>Good afternoon, Eternal.</h2>
          </div>
          <div className="topbar-actions">
            <span className="local-badge">Local-first</span>
            <button className="capture-button" onClick={captureContext} type="button" disabled={isCapturing}>
              {isCapturing ? "Saving context…" : "Add context"}
            </button>
          </div>
        </header>

        {notice && <div className="notice" role="status">{notice}</div>}

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
              <button className="panel-action" type="button" onClick={() => setActiveView("Projects")}>View all</button>
            </div>
            <div className="project-list">
              {snapshot.projects.map((project) => (
                <button className="project-row" key={project.id} type="button" onClick={() => setSnapshot((current) => ({ ...current, activeProject: project.name }))}>
                  <div className="project-name-block">
                    <span className="project-dot" aria-hidden="true" />
                    <div>
                      <strong>{project.name}</strong>
                      <span>{project.signal}</span>
                    </div>
                  </div>
                  <div className="project-progress-block">
                    <span>{project.status}</span>
                    <div className="progress-track" aria-label={`${project.progress}% complete`}>
                      <div className="progress-fill" style={{ width: `${project.progress}%` }} />
                    </div>
                  </div>
                  <time>{project.updatedAt}</time>
                </button>
              ))}
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
            <ol className="signal-list">
              {snapshot.signals.map((signal) => (
                <li className="signal-item" key={signal.id}>
                  <span className={`signal-icon ${signal.kind}`} aria-hidden="true">{signal.kind.slice(0, 1).toUpperCase()}</span>
                  <div>
                    <strong>{signal.title}</strong>
                    <p>{signal.detail}</p>
                    <time>{signal.time}</time>
                  </div>
                </li>
              ))}
            </ol>
          </article>
        </section>

        <section className="foundation-row" aria-label="Aura architecture status">
          <div className="foundation-copy">
            <p className="section-kicker">V0 FOUNDATION</p>
            <h3>Designed to earn trust before it automates.</h3>
            <p>
              Windows system awareness, project memory, and AI actions stay behind explicit permissions and readable local records.
            </p>
          </div>
          <div className="foundation-pillars">
            <span><b>01</b> Intentional perception</span>
            <span><b>02</b> Local memory contract</span>
            <span><b>03</b> Human-approved action</span>
          </div>
        </section>

        {isLoading && <div className="loading-line" aria-label="Loading local workspace state" />}
      </main>
    </div>
  );
}

export default App;
