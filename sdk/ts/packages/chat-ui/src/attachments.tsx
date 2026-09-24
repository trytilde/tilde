"use client";
import { useEffect, useRef, useState } from "react";
import { Button } from "@trytilde/connection-ui";
import type { Attachment } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import { type ChatClient, errorText } from "./client.js";
export function AttachmentView({
  client,
  attachment,
}: {
  client: ChatClient;
  attachment: Attachment;
}) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [requested, setRequested] = useState(false);
  const host = useRef<HTMLElement>(null);
  const downloadAfterLoad = useRef(false);
  const image = ["image/png", "image/jpeg", "image/gif", "image/webp", "image/avif"].includes(
    attachment.mediaType,
  );
  const media =
    image || attachment.mediaType.startsWith("audio/") || attachment.mediaType.startsWith("video/");
  useEffect(() => {
    if (!media) return;
    if (typeof IntersectionObserver === "undefined") {
      setRequested(true);
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setRequested(true);
        observer.disconnect();
      }
    });
    if (host.current) observer.observe(host.current);
    return () => observer.disconnect();
  }, [media]);
  useEffect(() => {
    if (!requested) return;
    const abort = new AbortController();
    let object = "";
    setUrl("");
    setError("");
    void client
      .downloadAttachment(
        { threadId: attachment.threadId, attachmentId: attachment.id },
        { signal: abort.signal },
      )
      .then((result) => {
        if (abort.signal.aborted) return;
        object = URL.createObjectURL(
          new Blob([new Uint8Array(result.content)], { type: attachment.mediaType }),
        );
        setUrl(object);
        if (downloadAfterLoad.current) {
          downloadAfterLoad.current = false;
          const link = document.createElement("a");
          link.href = object;
          link.download = attachment.filename;
          link.click();
        }
      })
      .catch((error) => {
        if (!abort.signal.aborted) setError(errorText(error));
      });
    return () => {
      abort.abort();
      if (object) URL.revokeObjectURL(object);
    };
  }, [
    client,
    attachment.id,
    attachment.threadId,
    attachment.mediaType,
    attachment.filename,
    retry,
    requested,
  ]);
  return (
    <figure className="tc-attachment" ref={host}>
      {url && image && <img src={url} alt={attachment.filename} loading="lazy" />}
      {url && attachment.mediaType.startsWith("audio/") && <audio controls src={url} />}
      {url && attachment.mediaType.startsWith("video/") && <video controls src={url} />}
      <figcaption>
        {url ? (
          <a href={url} download={attachment.filename}>
            {attachment.filename}
          </a>
        ) : (
          <>
            <span>
              {attachment.filename}
              {requested ? ` — ${error || "Loading…"}` : ""}
            </span>
            {!requested && (
              <Button
                onClick={() => {
                  downloadAfterLoad.current = !media;
                  setRequested(true);
                }}
              >
                {media ? "Load preview" : "Download"}
              </Button>
            )}
          </>
        )}
        {error && <Button onClick={() => setRetry((value) => value + 1)}>Retry download</Button>}
      </figcaption>
    </figure>
  );
}
