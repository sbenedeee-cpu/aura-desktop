import { render, screen, fireEvent, act } from "@testing-library/react";
import { describe, expect, it, vi, afterEach } from "vitest";
import { Overlay } from "./Overlay";

// The overlay tries to hide itself through the Tauri window API, which does
// not exist outside a Tauri runtime. Stub it so renderer tests stay pure.
const hideMock = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ hide: hideMock }),
}));

// EXP-007: the overlay now calls the native `run_brain` command. submit()
// is a plain async call (no internal setTimeout), so the suite no longer
// needs fake timers — promises resolve normally under act.
const flushPromises = () => new Promise((resolve) => setTimeout(resolve, 20));
const invokeMock = vi.fn().mockResolvedValue({
  tier: "deterministic",
  reply: 'Cortex heard: "hello aura" — floor answered.',
  degraded: false,
});

vi.mock("@tauri-apps/api/core", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tauri-apps/api/core")>()),
  invoke: (command: string, payload: unknown) => invokeMock(command, payload),
}));

describe("Neural Cortex overlay (EXP-005)", () => {
  afterEach(() => {
    hideMock.mockClear();
    invokeMock.mockClear();
  });

  it("renders the summon interface with an empty command line", () => {
    render(<Overlay />);
    expect(screen.getByRole("textbox", { name: /ask aura/i })).toBeInTheDocument();
    expect(screen.getByText("Neural Cortex")).toBeInTheDocument();
  });

  it("runs the brain and surfaces the reply after submit", async () => {
    render(<Overlay />);
    const input = screen.getByRole("textbox", { name: /ask aura/i });
    fireEvent.change(input, { target: { value: "hello aura" } });
    fireEvent.keyDown(input, { key: "Enter" });

    // The brain call is async — flush promises so the reply lands in the DOM.
    await act(async () => {
      await flushPromises();
    });

    expect(invokeMock).toHaveBeenCalledWith("run_brain", {
      input: { transcript: "hello aura", recentCaptures: [] },
    });
    expect(screen.getByText('Cortex heard: "hello aura" — floor answered.')).toBeInTheDocument();
  });

  it("surfaces a friendly error when the brain call fails", async () => {
    invokeMock.mockRejectedValueOnce(new Error("offline"));
    render(<Overlay />);
    const input = screen.getByRole("textbox", { name: /ask aura/i });
    fireEvent.change(input, { target: { value: "hello aura" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await act(async () => {
      await flushPromises();
    });
    expect(screen.getByText(/Cortex could not run the brain/)).toBeInTheDocument();
  });

  it("dismisses the overlay on Escape through the window API", async () => {
    render(<Overlay />);
    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape" });
      await flushPromises();
    });
    expect(hideMock).toHaveBeenCalled();
  });

  it("does not submit empty input", async () => {
    render(<Overlay />);
    fireEvent.keyDown(document, { key: "Enter" });
    await act(async () => {
      await flushPromises();
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
