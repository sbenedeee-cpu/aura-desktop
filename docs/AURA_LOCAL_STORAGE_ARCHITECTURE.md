# Aura V0 Local Storage & Privacy Architecture

This document outlines the design and architectural rules for Aura V0's persistent storage engine, schema migration strategies, failure recovery mechanisms, and the strict local privacy boundary.

---

## 1. Architectural Principles

- **Local-Only Boundary**: No user project, task, capture, or setting data leaves the user's local machine in V0.
- **Least Privilege Access**: The React renderer operates strictly in a zero-privileged state regarding the disk and system APIs. It can never handle direct database paths, raw SQL statements, or cryptographic keys.
- **Durable Local Records**: SQLite serves as the structured storage engine in the Rust native core.
- **Immutable Audit Trail**: User actions (such as starting, pausing, or changing settings) and intentional captures generate an append-only timeline/audit event trail.

---

## 2. Storage Directory & SQLite Schema

### 2.1 Storage Location
The application uses Tauri's secure path resolution to identify the platform-specific data directory:
- **Windows**: `C:\Users\<User>\AppData\Roaming\com.eternal.aura`
- **Linux**: `~/.config/com.eternal.aura`

The single file `aura.db` is stored inside this directory.

### 2.2 Core Schema Definitions
```sql
-- Schema Migration History
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY
);

-- Projects isolation boundary
CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  goal TEXT,
  status TEXT NOT NULL,
  current_task TEXT,
  blocker TEXT,
  next_step TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Flat tasks table
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id)
);

-- Immutable local event trail
CREATE TABLE events (
  id TEXT PRIMARY KEY,
  project_id TEXT,
  kind TEXT NOT NULL,
  actor TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  payload TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id)
);

-- Device & user settings
CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

---

## 3. Append-Only Schema Migrations & Transactional Guarantees

All SQLite schema modifications are managed in the Rust application layer:
1. **Migration Collection**: Managed as an ordered collection of migration steps.
2. **Current Version Discovery**: Querying the highest version from the `schema_migrations` table.
3. **Atomic Execution**: Apply missing migrations in a single, sequential transaction.
4. **Failure Rollback**: If any query or modification fails, the database automatically performs a complete rollback, keeping the schema version and file state safe and consistent.

---

## 4. Encryption & Key Management (Windows DPAPI)

Aura employs local key wrapping via the native **Windows Data Protection API (DPAPI)**:
1. **Data Encryption Key (DEK)**: A random 32-byte key is cryptographically generated on the first execution.
2. **Key Protection (DPAPI Wrapping)**: The raw DEK is protected/encrypted using `CryptProtectData` with the Windows user's local security context.
3. **Storage**: The wrapped DEK is stored locally.
4. **Unwrapping**: During startup, the core Rust backend calls `CryptUnprotectData` to retrieve the active DEK for opening the SQLite database with SQLCipher.
5. **No Key Leakage**: Raw key material or DEK byte representation is never written to logs, stored in the browser/renderer process, or exposed to TypeScript state.

---

## 5. Corruption Recovery Path

Aura protects user data by implementing a robust failure recovery mechanism:
1. **Initialization Health Check**: On startup, the Rust backend attempts to open `aura.db` and query schema details.
2. **Corruption Detection**: If the file header is corrupted or key decryption fails, the database loader intercepts the error.
3. **Backup Preservation**: The application copies the existing `aura.db` to `aura.db.bak` to preserve any potentially recoverable raw user bytes.
4. **Safe Reset**: The original database path is cleared, and a clean, freshly migrated database is bootstrapped so the desktop shell remains operational.

---

## 6. The Strict Privacy & Least Privilege Boundary

```
 +-----------------------------------------------------------------------------------------+
 |                                    React Frontend                                       |
 |  - No SQLite / SQLCipher libraries imported in Vite build                               |
 |  - No direct SQL queries, DB path references, or key management symbols                 |
 |  - Communication limited strictly to typed, schema-validated Tauri commands            |
 +-----------------------------------------------------------------------------------------+
                                              |
                                              | (Tauri IPC Bridge / JSON-RPC Command Contract)
                                              v
 +-----------------------------------------------------------------------------------------+
 |                                  Rust Native Backend                                    |
 |  - Handles DPAPI wrapping/unwrapping                                                    |
 |  - Direct filesystem and SQLite database ownership                                      |
 |  - Validates and enforces Privacy Modes (Focused / Paused) before any DB mutation       |
 +-----------------------------------------------------------------------------------------+
```

### 6.1 Core Security & Verification Checklist
- **No Direct DB Access**: Ensure that the renderer contains no `sqlite3`, `sqlite`, or database connection code.
- **No Secret Exposure**: Do not let standard logs print query params or key strings.
- **Explicit Privacy Invariant**: If privacy mode is set to `paused`, Tauri command handlers must explicitly block database write transactions for intentional capture.
