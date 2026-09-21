export type Status =
  | "waiting"
  | "seen"
  | "receiving"
  | "sent"
  | "received"
  | "failed"
  | "cancelled"
  | "retry_requested"
  | "expired";
export interface User {
  login: string;
  name?: string | null;
}
export interface Chat {
  repo: string;
  name: string;
  members: string[];
}
export interface SharedFile {
  name: string;
  size: number;
  directory: boolean;
}
export interface QueuedFile extends SharedFile {
  path: string;
}
export interface Delivery {
  recipient: string;
  attempt: number;
  status: Status;
  expires_at: number;
}
export interface Transfer {
  key: string;
  repo: string;
  issue: number;
  sender: string;
  source_device: string;
  device_name: string;
  created_at: number;
  files: SharedFile[];
  deliveries: Delivery[];
  can_retry: boolean;
  error?: string | null;
}
export interface Job {
  id: string;
  key: string;
  recipient: string;
  direction: "send" | "receive";
}
export interface Invitation {
  id: number;
  repository: { full_name: string; description: string | null };
}
export interface Snapshot {
  user: User | null;
  connected: boolean;
  platform: string;
  device_id: string;
  device_name: string;
  chats: Chat[];
  contacts: string[];
  transfers: Transfer[];
  jobs: Job[];
  invitations: Invitation[];
  pending_receipts: number;
  last_sync: number | null;
  error: string | null;
  croc_version: string | null;
  oauth_available: boolean;
}
export const statusText: Record<Status, string> = {
  waiting: "Waiting",
  seen: "Seen",
  receiving: "Receiving",
  sent: "Sent",
  received: "Received",
  failed: "Failed",
  cancelled: "Cancelled",
  retry_requested: "Retry requested",
  expired: "Timed out",
};
export const needsRetry = (s: Status) =>
  ["failed", "cancelled", "retry_requested", "expired"].includes(s);
export function sizeLabel(bytes: number): string {
  if (!bytes) return "0 B";
  const n = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 3);
  return `${(bytes / 1024 ** n).toFixed(n === 0 ? 0 : 1)} ${["B", "KB", "MB", "GB"][n]}`;
}
export const sameUser = (a?: string, b?: string) =>
  Boolean(a && b && a.toLowerCase() === b.toLowerCase());
export function incomingFor(t: Transfer, state: Snapshot) {
  return (
    t.source_device !== state.device_id &&
    t.deliveries.some((d) => sameUser(d.recipient, state.user?.login))
  );
}
export function chatSummary(chat: Chat, state: Snapshot): string {
  const transfers = state.transfers
    .filter((t) => t.repo === chat.repo)
    .sort((a, b) => b.created_at - a.created_at);
  const latest = transfers[0];
  if (!latest) return chat.members.length === 1 ? "My devices" : "No files yet";
  if (latest.deliveries.some((d) => needsRetry(d.status)))
    return "Retry needed";
  if (latest.deliveries.every((d) => d.status === "received"))
    return `${latest.files.length} ${latest.files.length === 1 ? "item" : "items"} received`;
  return incomingFor(latest, state) ? "Files waiting" : "Sent";
}

export function parseMembers(value: string): string[] {
  return [
    ...new Map(
      value
        .split(/[\s,]+/)
        .map((s) => s.replace(/^@/, ""))
        .filter(Boolean)
        .map((s) => [s.toLowerCase(), s]),
    ).values(),
  ];
}
