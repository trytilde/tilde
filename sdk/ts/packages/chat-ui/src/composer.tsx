// Adapted from trytilde/dispatch packages/ui. See ../LICENSE.dispatch.
"use client";
import { Button, Textarea } from "@trytilde/connection-ui";
import {
  useEffect,
  useRef,
  useState,
  type ChangeEvent,
  type DragEvent,
  type FocusEventHandler,
  type FormEventHandler,
  type RefObject,
} from "react";
import { PlusIcon, ReplyIcon, SendIcon } from "lucide-react";

export interface ComposerAttachment {
  id: string;
  name: string;
  size: number;
  progress: number;
  status: "ready" | "uploading" | "uploaded" | "error";
  error?: string;
  /** Local blob URL for image previews while the upload is pending. */
  previewUrl?: string;
}

export interface ComposerReply {
  label: string;
  text: string;
}

export interface ChatComposerProps {
  agentAvailable: boolean;
  busy: boolean;
  submitting: boolean;
  dragging: boolean;
  expanded: boolean;
  draft: string;
  error?: string;
  reply?: ComposerReply;
  attachments: readonly ComposerAttachment[];
  inputRef: RefObject<HTMLTextAreaElement | null>;
  fileInputRef: RefObject<HTMLInputElement | null>;
  onSubmit: FormEventHandler<HTMLFormElement>;
  onDraftChange: (value: string) => void;
  onFocus?: FocusEventHandler<HTMLTextAreaElement>;
  onBlur?: FocusEventHandler<HTMLTextAreaElement>;
  onDragStateChange: (active: boolean) => void;
  onFilesAdded: (files: FileList) => void;
  onRemoveAttachment: (id: string) => void;
  onCancelReply: () => void;
  onStop: () => void;
}

export interface ComposerKeySubmission {
  key: string;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  composing: boolean;
  coarsePointer: boolean;
}

/** Desktop Enter sends; touch keyboards keep Return available for multiline drafting. */
export function shouldSubmitComposerKey(input: ComposerKeySubmission): boolean {
  if (input.key !== "Enter" || input.composing) return false;
  if (input.metaKey || input.ctrlKey) return true;
  return !input.shiftKey && !input.coarsePointer;
}

export function ChatComposer({
  agentAvailable,
  busy,
  submitting,
  dragging,
  expanded,
  draft,
  error = "",
  reply,
  attachments,
  inputRef,
  fileInputRef,
  onSubmit,
  onDraftChange,
  onFocus,
  onBlur,
  onDragStateChange,
  onFilesAdded,
  onRemoveAttachment,
  onCancelReply,
  onStop,
}: ChatComposerProps) {
  const hasContent = Boolean(draft.trim() || attachments.length);
  const [attachmentMenuOpen, setAttachmentMenuOpen] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);

  useEffect(() => {
    if (!attachmentMenuOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!formRef.current?.contains(event.target as Node)) setAttachmentMenuOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAttachmentMenuOpen(false);
    };
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [attachmentMenuOpen]);

  function setDrag(event: DragEvent, active: boolean): void {
    event.preventDefault();
    onDragStateChange(active);
  }

  return (
    <div className="composer-shell">
      {error ? (
        <div className="composer-error-pill" role="alert">
          <span aria-hidden="true" className="composer-error-dot" />
          <span>{error}</span>
        </div>
      ) : null}
      <form
        ref={formRef}
        className={`composer ${dragging ? "dragging" : ""} ${expanded ? "expanded" : ""}`}
        onSubmit={onSubmit}
        onDragEnter={(event) => setDrag(event, true)}
        onDragOver={(event) => setDrag(event, true)}
        onDragLeave={(event) => setDrag(event, false)}
        onDrop={(event) => {
          event.preventDefault();
          onDragStateChange(false);
          onFilesAdded(event.dataTransfer.files);
        }}
      >
        {reply ? (
          <div className="reply-preview">
            <ReplyIcon />
            <span>
              <strong>{reply.label}</strong>
              <small>{reply.text}</small>
            </span>
            <Button
              aria-label="Cancel reply"
              onClick={onCancelReply}
              type="button"
              disabled={submitting}
            >
              ×
            </Button>
          </div>
        ) : null}
        {attachments.length ? (
          <div className="attachment-tray">
            {attachments.map((attachment) => (
              <div className={`pending-file ${attachment.status}`} key={attachment.id}>
                {attachment.previewUrl ? (
                  <img alt="" className="file-thumb" src={attachment.previewUrl} />
                ) : (
                  <span className="attachment-glyph">↗</span>
                )}
                <span>
                  <strong>{attachment.name}</strong>
                  <small>
                    {attachment.status === "uploading"
                      ? "Uploading…"
                      : attachment.error || formatBytes(attachment.size)}
                  </small>
                </span>
                <Button
                  type="button"
                  onClick={() => onRemoveAttachment(attachment.id)}
                  aria-label="Remove file"
                  disabled={submitting}
                >
                  ×
                </Button>
                {attachment.status === "uploading" ? (
                  <i style={{ width: `${attachment.progress * 100}%` }} />
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
        {dragging ? <div className="drop-overlay">Drop files to attach</div> : null}
        {attachmentMenuOpen ? (
          <div className="composer-attachment-menu" role="menu" aria-label="Add to message">
            <Button
              role="menuitem"
              type="button"
              onClick={() => {
                setAttachmentMenuOpen(false);
                fileInputRef.current?.click();
              }}
            >
              <span className="composer-attachment-menu-icon" aria-hidden="true">
                ↗
              </span>
              <span>
                <strong>Add photos &amp; files</strong>
                <small>Upload from your computer</small>
              </span>
            </Button>
          </div>
        ) : null}
        <div className="composer-input-grid">
          <div className="composer-attachment-control">
            <input
              hidden
              multiple
              ref={fileInputRef}
              type="file"
              onChange={(event: ChangeEvent<HTMLInputElement>) => {
                if (event.target.files) onFilesAdded(event.target.files);
                event.target.value = "";
              }}
            />
            <Button
              className="attach-button"
              type="button"
              disabled={!agentAvailable || submitting}
              aria-expanded={attachmentMenuOpen}
              aria-haspopup="menu"
              onClick={() => setAttachmentMenuOpen((open) => !open)}
              aria-label="Add photos and files"
              title="Add photos and files"
            >
              <PlusIcon />
            </Button>
          </div>
          <Textarea
            aria-label="Message"
            disabled={!agentAvailable || submitting}
            ref={inputRef}
            placeholder={
              agentAvailable
                ? busy
                  ? "Write another message to queue…"
                  : "Write a message…"
                : "No agent is available."
            }
            value={draft}
            onChange={(event) => onDraftChange(event.target.value)}
            onPaste={(event) => {
              if (!submitting && event.clipboardData.files.length) {
                event.preventDefault();
                onFilesAdded(event.clipboardData.files);
              }
            }}
            onBlur={onBlur}
            onFocus={onFocus}
            onKeyDown={(event) => {
              const coarsePointer =
                event.currentTarget.ownerDocument.defaultView?.matchMedia?.("(pointer: coarse)")
                  .matches ?? false;
              if (
                shouldSubmitComposerKey({
                  key: event.key,
                  shiftKey: event.shiftKey,
                  metaKey: event.metaKey,
                  ctrlKey: event.ctrlKey,
                  composing: event.nativeEvent.isComposing,
                  coarsePointer,
                })
              ) {
                event.preventDefault();
                event.currentTarget.form?.requestSubmit();
              }
            }}
          />
          <div className="composer-actions">
            {busy ? (
              <Button className="stop-button" type="button" onClick={onStop} aria-label="Stop">
                <span aria-hidden="true" className="composer-stop-glyph" />
              </Button>
            ) : null}
            <Button
              type="submit"
              aria-label="Send message"
              disabled={!agentAvailable || !hasContent || submitting}
            >
              <SendIcon />
            </Button>
          </div>
        </div>
      </form>
    </div>
  );
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}
