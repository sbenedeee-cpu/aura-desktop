import { render, screen, fireEvent, act } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { Overlay } from "./Overlay";

// The overlay tries to hide itself through the Tauri window API, which does
// not exist outside a Tauri runtime. Stub it so renderer tests stay pure.
const hideMock = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ hide: hideMock }),
}));

describe("Neural Cortex overlay (EXP-005)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    hideMock.mockClear();
  });

  it("renders the summon interface with an empty command line", () => {
    render(<Overlay />);
    expect(screen.getByRole("textbox", { name: /ask aura/i })).toBeInTheDocument();
    expect(screen.getByText("Neural Cortex")).toBeInTheDocument();
  });

  it("echoes a typed message after the thinking delay", async () => {
    render(<Overlay />);
    const input = screen.getByRole("textbox", { name: /ask aura/i });
    fireEvent.change(input, { target: { value: "hello aura" } });
    fireEvent.keyDown(input, { key: "Enter" });

    // Under fake timers, advanceTimersByTimeAsync both advances timers and
    // flushes the React microtask queue so state updates land in the DOM.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(450);
    });

    expect(screen.getByText(/Cortex heard: "hello aura"/)).toBeInTheDocument();
  });

  it("dismisses the overlay on Escape through the window API", async () => {
    render(<Overlay />);
    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape" });
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(hideMock).toHaveBeenCalled();
  });

  it("does not submit empty input", () => {
    render(<Overlay />);
    fireEvent.keyDown(document, { key: "Enter" });
    vi.advanceTimersByTime(450);
    expect(screen.queryByText(/Cortex heard/)).not.toBeInTheDocument();
  });
});
