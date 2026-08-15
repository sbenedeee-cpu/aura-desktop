import { useCallback, useEffect, useRef, useState } from "react";

/**
 * EXP-005: Neural Cortex overlay.
 *
 * The hotkey-summoned assistant interface. For this increment the overlay is
 * a focused input surface with an echo brain: whatever is typed is repeated
 * back with a small thinking delay, proving the summon loop end-to-end.
 * Voice (push-to-talk) arrives in EXP-006 and the real brain in EXP-007.
 *
 * Visual identity (assistant-first, separate from the continuity desk):
 * dark translucent surface, single focused command line, subtle aura glow,
 * no navigation chrome. Esc closes the overlay and returns focus to the
 * previous application.
 */
export function Overlay() {
  const [input, setInput] = useState("");
  const [reply, setReply] = useState("");
  const [isThinking, setIsThinking] = useState(false);
  const [isClosing, setIsClosing] = useState(false);
  const [micPlaceholder] = useState("Voice arrives in the next build.");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Ensure the command line owns focus the moment the overlay appears.
    inputRef.current?.focus();
  }, []);

  const dismiss = useCallback(async () => {
    if (isClosing) return;
    setIsClosing(true);
    try {
      // The window plugin ships with the Tauri 2 template; overlay hides via
      // the Rust command in EXP-006. For now a graceful fade-out is the
      // entire contract and the next increment wires window.hide().
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      void getCurrentWindow().hide();
    } catch {
      // Fallback: hide the root element so the overlay looks closed even if
      // the window API is unavailable in development previews.
      document.getElementById("root")?.setAttribute("hidden", "hidden");
    }
  }, [isClosing]);

  const submit = useCallback(async () => {
    const value = input.trim();
    if (!value || isThinking) return;
    setIsThinking(true);
    setReply("");
    // Echo brain for EXP-005: the Neural Cortex brain (EXP-007) replaces this
    // delay-and-echo with real local/cloud intent execution.
    const timer = window.setTimeout(() => {
      setReply(`Cortex heard: "${value}" — full reasoning arrives in the next build.`);
      setIsThinking(false);
      inputRef.current?.focus();
    }, 450);
    return () => window.clearTimeout(timer);
  }, [input, isThinking]);

  const handleSubmit = useCallback(() => {
    void submit();
  }, [submit]);

  const handleDismiss = useCallback(() => {
    void dismiss();
  }, [dismiss]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        handleDismiss();
      } else if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        handleSubmit();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [handleDismiss, handleSubmit]);

  return (
    <div className="aura-overlay">
      <header className="aura-overlay-brand" aria-label="Neural Cortex">
        <span className="aura-orb" aria-hidden="true" />
        <span className="aura-brand-line">
          <strong>Aura</strong> <small>Neural Cortex</small>
        </span>
      </header>

      <div className="aura-overlay-body">
        <form className="aura-command-line" onSubmit={handleSubmit}>
          <span className="aura-prompt" aria-hidden="true">
            &gt;
          </span>
          <input
            ref={inputRef}
            aria-label="Ask Aura"
            autoComplete="off"
            autoFocus
            className="aura-command-input"
            onChange={(event) => setInput(event.target.value)}
            placeholder="Ask Aura anything… (Alt+Space to summon, Esc to dismiss)"
            type="text"
            value={input}
          />
          <button
            aria-label="Send message"
            className="aura-send"
            disabled={isThinking || !input.trim()}
            type="submit"
          >
            {isThinking ? (
              <span className="aura-spinner" aria-hidden="true" />
            ) : (
              <svg aria-hidden="true" className="aura-icon" viewBox="0 0 24 24">
                <path
                  d="M4 12h13M12 6l6 6-6 6"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            )}
          </button>
        </form>

        <div aria-live="polite" className="aura-reply">
          {reply && <p className="aura-reply-text">{reply}</p>}
        </div>

        <footer className="aura-overlay-foot">
          <span className="aura-mic" title="Voice input — push-to-talk arrives in EXP-006">
            <svg aria-hidden="true" className="aura-icon" viewBox="0 0 24 24">
              <rect x="9" y="3" width="6" height="11" rx="3" fill="currentColor" opacity="0.9" />
              <path
                d="M6 11a6 6 0 0 0 12 0M12 17v4M9 21h6"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
            <small>{micPlaceholder}</small>
          </span>
        </footer>
      </div>
    </div>
  );
}
