// Markup from trytilde/dispatch packages/ui/src/activity-queue.tsx (a655a4f). See ../LICENSE.dispatch.
// Native drag and drop replaces Dispatch's motion reorder; up/down keep reordering keyboard-reachable.
import type { QueuedInput } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  CornerDownRightIcon,
  GripVerticalIcon,
  PencilIcon,
  Trash2Icon,
} from "lucide-react";

export function QueuePanel({
  items,
  onMove,
  onSteer,
  onEdit,
  onRemove,
}: {
  items: readonly QueuedInput[];
  /** Moves `id` before `beforeId`; an empty `beforeId` moves it to the end. */
  onMove: (id: string, beforeId: string) => void;
  onSteer: (id: string) => void;
  onEdit: (item: QueuedInput) => void;
  onRemove: (id: string) => void;
}) {
  return (
    <section aria-label="Queued messages" className="queue-panel" data-variant="embedded">
      <div className="queue-viewport">
        <ol className="queue-list">
          {items.map((item, index) => (
            <li
              key={item.id}
              className="queue-item"
              draggable
              onDragStart={(event) => event.dataTransfer.setData("text/tilde-queue", item.id)}
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                event.preventDefault();
                const id = event.dataTransfer.getData("text/tilde-queue");
                if (id !== item.id && items.some((queued) => queued.id === id)) onMove(id, item.id);
              }}
            >
              <span aria-hidden="true" className="queue-grip">
                <GripVerticalIcon aria-hidden="true" />
              </span>
              <span className="queue-message">{item.text}</span>
              <button
                aria-label="Move queued message up"
                className="queue-edit"
                disabled={index === 0}
                onClick={() => onMove(item.id, items[index - 1].id)}
                type="button"
              >
                <ArrowUpIcon aria-hidden="true" />
              </button>
              <button
                aria-label="Move queued message down"
                className="queue-edit"
                disabled={index === items.length - 1}
                onClick={() => onMove(item.id, items[index + 2]?.id ?? "")}
                type="button"
              >
                <ArrowDownIcon aria-hidden="true" />
              </button>
              <button
                aria-label="Steer queued message"
                className="queue-steer"
                onClick={() => onSteer(item.id)}
                type="button"
              >
                <CornerDownRightIcon aria-hidden="true" />
                <span>Steer</span>
              </button>
              <button
                aria-label="Delete queued message"
                className="queue-delete"
                onClick={() => onRemove(item.id)}
                type="button"
              >
                <Trash2Icon aria-hidden="true" />
              </button>
              <button
                aria-label="Edit queued message"
                className="queue-edit"
                onClick={() => onEdit(item)}
                type="button"
              >
                <PencilIcon aria-hidden="true" />
              </button>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
