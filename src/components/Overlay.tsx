import { useCallback, useEffect, useRef, useState } from "react";

import { usePushToTalk } from "./usePushToTalk";

/**
 * EXP-005 / EXP-006: Neural Cortex overlay.
 *
 * The hotkey-summoned assistant interface. EXP-005 proved the summon loop
 * with a focused input and an echo brain; EXP-006 adds push-to-talk — hold
 * the Space bar inside the overlay (or click the mic) to dictate, and the
 * transcript flows into the command line. The full reasoning brain arrives
 * in EXP-007.
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
  const inputRef = useRef<HTMLInputElement>(null);
  const spaceHeldRef = useRef(false);

  const { error: voiceError, startListening, status, stopListening } = usePushToTalk();

  const isListening = status === "listening";
  const isTranscribing = status === "transcribing";
  const isRecording = isListening || isTranscribing;

  useEffect(() => {
    // Ensure the command line owns focus the moment the overlay appears.
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    // When dictation completes, drop the transcript into the command line and
    // dispatch it immediately — speaking already counts as submitting.
    const onTranscript = (event: Event) => {
      const text = (event as CustomEvent<string>).detail?.trim();
      if (!text) return;
      setInput((current) => (current ? `${current} ${text}` : text));
      inputRef.current?.focus();
    };
    window.addEventListener("aura-voice-transcript", onTranscript);
    return () => window.removeEventListener("aura-voice-transcript", onTranscript);
  }, []);

  const dismiss = useCallback(async () => {
    if (isClosing) return;
    setIsClosing(true);
    stopListening();
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      void getCurrentWindow().hide();
    } catch {
      // Fallback: hide the root element so the overlay looks closed even if
      // the window API is unavailable in development previews.
      document.getElementById("root")?.setAttribute("hidden", "hidden");
    }
  }, [isClosing, stopListening]);

  // EXP-007: the real brain replaces the echo. The transcript goes through
  // the native tiered engine (cloud → local ollama → deterministic floor)
  // and the reply is surfaced with the tier that actually answered.
  const submit = useCallback(async () => {
    const value = input.trim();
    if (!value || isThinking || isTranscribing) return;
    setIsThinking(true);
    setReply("");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<{ tier: string; reply: string; degraded: boolean }>("run_brain", {
        input: { transcript: value, recentCaptures: [] },
      });
      const tierTag =
        result.tier === "cloud" ? " ☁ cloud" : result.tier === "local" ? " ● local" : " ◂ floor";
      setReply(result.reply + (result.degraded ? ` (${tierTag})` : ""));
    } catch (error) {
      setReply(`Cortex could not run the brain: ${error}`);
    } finally {
      setIsThinking(false);
      inputRef.current?.focus();
    }
    return () => {
      /* no-op cleanup kept for the existing submit() contract */
    };
  }, [input, isThinking, isTranscribing]);

  const handleSubmit = useCallback(() => {
    void submit();
  }, [submit]);

  const handleDismiss = useCallback(() => {
    void dismiss();
  }, [dismiss]);

  const handleMicClick = useCallback(async () => {
    if (isRecording) {
      stopListening();
      return;
    }
    const microphoneError = await startListening();
    if (microphoneError) setReply(microphoneError);
  }, [isRecording, startListening, stopListening]);

  // The hold-to-talk contract: Space while the input owns focus records; a
  // held Space never types into the command line. Enter still submits.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const isInputFocused =
        target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
      if (event.key === "Escape") {
        handleDismiss();
        return;
      }
      if (event.key === " " && isInputFocused && !spaceHeldRef.current) {
        event.preventDefault();
        spaceHeldRef.current = true;
        void startListening().then((microphoneError) => {
          if (microphoneError) setReply(microphoneError);
        });
      } else if (event.key === "Enter" && !event.shiftKey && !isRecording) {
        event.preventDefault();
        handleSubmit();
      }
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key === " " && spaceHeldRef.current) {
        event.preventDefault();
        spaceHeldRef.current = false;
        stopListening();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("keyup", onKeyUp);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("keyup", onKeyUp);
    };
  }, [handleDismiss, handleSubmit, isRecording, startListening, stopListening]);

  return (
    <div className="aura-overlay" data-voice={isRecording ? "active" : undefined}>
      <header className="aura-overlay-brand" aria-label="Neural Cortex">
        <span className="aura-orb" aria-hidden="true" data-pulse={isListening ? "on" : undefined} />
        <span className="aura-brand-line">
          <strong>Aura</strong> <small>Neural Cortex</small>
        </span>
        {voiceError && (
          <small className="aura-voice-error" role="alert">
            {voiceError}
          </small>
        )}
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
            placeholder={
              isTranscribing
                ? "Listening… Cortex is transcribing…"
                : "Ask Aura anything… (Alt+Space to summon, hold Space to dictate)"
            }
            type="text"
            value={input}
          />
          <button
            aria-label={isRecording ? "Stop recording" : "Start voice input"}
            className={`aura-send${isListening ? " aura-send-listening" : ""}`}
            disabled={isThinking || (!input.trim() && !isRecording)}
            onClick={handleMicClick}
            type="button"
          >
            {isTranscribing ? (
              <span className="aura-spinner" aria-hidden="true" />
            ) : (
              <svg aria-hidden="true" className="aura-icon" viewBox="0 0 24 24">
                {isRecording ? (
                  <rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor" />
                ) : (
                  <path
                    d="M4 12h13M12 6l6 6-6 6"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                )}
              </svg>
            )}
          </button>
        </form>

        <div aria-live="polite" className="aura-reply">
          {reply && <p className="aura-reply-text">{reply}</p>}
        </div>

        <footer className="aura-overlay-foot">
          <button
            aria-label={isRecording ? "Stop recording" : "Push-to-talk: hold Space or click"}
            className="aura-mic"
            disabled={isThinking}
            onClick={handleMicClick}
            type="button"
            title="Push-to-talk: hold Space while typing, or click"
          >
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
            <small>
              {isTranscribing
                ? "Transcribing on your machine…"
                : isListening
                  ? "Listening… release Space to send"
                  : "Voice input"}
            </small>
          </button>
        </footer>
      </div>
    </div>
  );
}
