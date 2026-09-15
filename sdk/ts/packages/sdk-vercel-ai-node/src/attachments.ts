import type { AgentContext, Attachment, ConversationMessage } from "@trytilde/sdk";
import type { UIMessage } from "ai";

export type AttachmentConversion = {
  message: ConversationMessage;
  attachment: Attachment;
  /** Uses the current invocation's authenticated, thread-scoped attachment API. */
  download: () => ReturnType<AgentContext["attachments"]["download"]>;
};
export type AttachmentHandler = (
  input: AttachmentConversion,
) =>
  | UIMessage["parts"][number]
  | UIMessage["parts"][number][]
  | null
  | Promise<UIMessage["parts"][number] | UIMessage["parts"][number][] | null>;

/** Hydrate images/PDFs as file parts and textual files as actual text, not filenames. */
export const convertAttachment: AttachmentHandler = async ({ attachment, download }) => {
  const mediaType = normalizeMediaType(attachment.mediaType, attachment.filename);
  const text =
    mediaType.startsWith("text/") || ["application/json", "application/xml"].includes(mediaType);
  const file = ["image/jpeg", "image/png", "image/gif", "image/webp", "application/pdf"].includes(
    mediaType,
  );
  if (!text && !file) {
    return {
      type: "text",
      text: `Attached file: ${attachment.filename} (${mediaType}). A custom attachment handler is needed to read this format.`,
    };
  }
  const result = await download();
  if (result.attachment?.id !== attachment.id)
    throw new Error("Attachment download returned a different file");
  const bytes = Buffer.from(result.content);
  if (text)
    return {
      type: "text",
      text: `Attached file: ${attachment.filename}\n${bytes.toString("utf8")}`,
    };
  return {
    type: "file",
    mediaType,
    filename: attachment.filename,
    url: `data:${mediaType};base64,${bytes.toString("base64")}`,
  };
};

function normalizeMediaType(value: string, filename: string): string {
  const type = value.split(";")[0].trim().toLowerCase();
  if (type && type !== "application/octet-stream") return type;
  const extension = filename.split(".").at(-1)?.toLowerCase();
  switch (extension) {
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    case "png":
      return "image/png";
    case "gif":
      return "image/gif";
    case "webp":
      return "image/webp";
    case "pdf":
      return "application/pdf";
    case "txt":
    case "md":
    case "csv":
    case "log":
      return "text/plain";
    case "json":
      return "application/json";
    case "xml":
      return "application/xml";
    default:
      return type || "application/octet-stream";
  }
}
