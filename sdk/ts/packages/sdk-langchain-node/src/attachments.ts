import type { AgentContext, Attachment, ConversationMessage } from "@trytilde/sdk";
import type { ContentBlock } from "@langchain/core/messages";

/** Standard LangChain content blocks: text, plus base64 image/file data for multimodal models. */
export type AttachmentBlock =
  | ContentBlock.Text
  | ContentBlock.Multimodal.Image
  | ContentBlock.Multimodal.File;
export type AttachmentConversion = {
  message: ConversationMessage;
  attachment: Attachment;
  /** Uses the current invocation's authenticated, thread-scoped attachment API. */
  download: () => ReturnType<AgentContext["attachments"]["download"]>;
};
export type AttachmentHandler = (
  input: AttachmentConversion,
) =>
  | AttachmentBlock
  | AttachmentBlock[]
  | null
  | Promise<AttachmentBlock | AttachmentBlock[] | null>;

/** Hydrate images/PDFs as base64 blocks and textual files as actual text, not filenames. */
export const convertAttachment: AttachmentHandler = async ({ attachment, download }) => {
  const mediaType = normalizeMediaType(attachment.mediaType, attachment.filename);
  const text =
    mediaType.startsWith("text/") || ["application/json", "application/xml"].includes(mediaType);
  const image = ["image/jpeg", "image/png", "image/gif", "image/webp"].includes(mediaType);
  if (!text && !image && mediaType !== "application/pdf") {
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
  if (image) return { type: "image", mimeType: mediaType, data: bytes.toString("base64") };
  return {
    type: "file",
    mimeType: mediaType,
    data: bytes.toString("base64"),
    metadata: { filename: attachment.filename },
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
