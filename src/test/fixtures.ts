// TypeScript DTO Mock Fixtures for Frontend Verification

export interface ProjectDTO {
  id: string;
  name: string;
  goal: string;
  status: string;
  current_task: string;
  blocker: string;
  next_step: string;
  createdAt: string;
  updatedAt: string;
}

export interface TaskDTO {
  id: string;
  projectId: string;
  title: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}

export interface EventDTO {
  id: string;
  projectId?: string;
  kind: string;
  actor: string;
  occurredAt: string;
  payload: string;
}

export const mockProjectDTO: ProjectDTO = {
  id: "aura",
  name: "Aura Desktop",
  goal: "Convert validated architecture shell into a trustworthy local-first system",
  status: "In progress",
  current_task: "Implement safe DB persistence and tests",
  blocker: "Awaiting approval of ADR-003",
  next_step: "Establish local SQLite database layer",
  createdAt: "2026-08-12T14:00:00Z",
  updatedAt: "2026-08-12T15:30:00Z",
};

export const mockTaskDTO: TaskDTO = {
  id: "task-001",
  projectId: "aura",
  title: "Setup rusqlite and migrations",
  status: "todo",
  createdAt: "2026-08-12T14:10:00Z",
  updatedAt: "2026-08-12T14:10:00Z",
};

export const mockEventDTO: EventDTO = {
  id: "event-101",
  projectId: "aura",
  kind: "CAPTURE_CREATED",
  actor: "user",
  occurredAt: "2026-08-12T14:15:00Z",
  payload: JSON.stringify({ type: "note", title: "Local DB design approved" }),
};
