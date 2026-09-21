import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Chat, Delivery, SharedFile, Snapshot, Transfer } from "./types";

export const desktop = isTauri();
const now = () => Math.floor(Date.now() / 1000);
const device = "preview-laptop";
const receipt = (recipient: string, status: Delivery["status"]): Delivery => ({
  recipient,
  status,
  attempt: 1,
  expires_at: now() + 900,
});
const file = (name: string, size: number): SharedFile => ({
  name,
  size,
  directory: false,
});
const chats: Chat[] = [
  { repo: "you/cricket-maya", name: "Maya Chen", members: ["you", "mayachen"] },
  { repo: "you/cricket-devices", name: "My devices", members: ["you"] },
  {
    repo: "you/cricket-weekend",
    name: "Weekend crew",
    members: ["you", "mayachen", "oliver", "jules"],
  },
];
const transfer = (
  issue: number,
  sender: string,
  files: SharedFile[],
  deliveries: Delivery[],
  minutes: number,
  repo = chats[0].repo,
): Transfer => ({
  key: `${repo}#${issue}`,
  repo,
  issue,
  sender,
  source_device: sender === "you" ? device : "preview-remote",
  device_name: sender === "you" ? "Windows laptop" : "MacBook Air",
  created_at: now() - minutes * 60,
  files,
  deliveries,
  can_retry: sender === "you",
});
let preview: Snapshot = {
  user: { login: "you", name: "Alex Morgan" },
  connected: true,
  platform: "windows",
  device_id: device,
  device_name: "Windows laptop",
  chats,
  contacts: ["jules", "mayachen", "oliver"],
  transfers: [
    transfer(
      1,
      "you",
      [file("weekend-photos.zip", 24_400_000)],
      [receipt("mayachen", "received")],
      68,
    ),
    transfer(
      2,
      "mayachen",
      [
        file("little-moments.jpg", 4_200_000),
        file("by-the-water.jpg", 3_800_000),
      ],
      [receipt("you", "received")],
      57,
    ),
    transfer(
      3,
      "mayachen",
      [file("the-good-stuff.zip", 18_600_000)],
      [receipt("you", "waiting")],
      2,
    ),
    transfer(
      1,
      "you",
      [file("camping-checklist.pdf", 284_000)],
      [
        receipt("mayachen", "received"),
        receipt("oliver", "received"),
        receipt("jules", "expired"),
      ],
      125,
      chats[2].repo,
    ),
    {
      ...transfer(
        1,
        "you",
        [file("project-backup.zip", 12_800_000)],
        [receipt("you", "waiting")],
        5,
        chats[1].repo,
      ),
      source_device: "preview-desktop",
      device_name: "Linux desktop",
      can_retry: false,
    },
  ],
  jobs: [],
  invitations: [],
  pending_receipts: 0,
  last_sync: now(),
  error: null,
  croc_version: "croc version v11.5.3",
  oauth_available: false,
};
const change = () => window.dispatchEvent(new Event("cricket-preview"));
export async function call<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (desktop) return invoke<T>(command, args);
  await new Promise((resolve) => setTimeout(resolve, 160));
  let value: unknown;
  switch (command) {
    case "snapshot":
      value = structuredClone(preview);
      break;
    case "sync":
      preview.last_sync = now();
      break;
    case "mark_seen":
      break;
    case "create_chat": {
      const chat: Chat = {
        repo: `you/cricket-${crypto.randomUUID().slice(0, 8)}`,
        name: String(args.name),
        members: ["you", ...(args.members as string[]).filter(Boolean)],
      };
      preview.chats.push(chat);
      preview.contacts = [
        ...new Map(
          [...preview.contacts, ...chat.members.filter((m) => m !== "you")].map(
            (m) => [m.toLowerCase(), m],
          ),
        ).values(),
      ].sort();
      value = chat;
      break;
    }
    case "import_chat":
      throw new Error(
        "Open the desktop app to connect a real GitHub repository.",
      );
    case "send_files": {
      const chat = preview.chats.find((c) => c.repo === args.repo)!;
      const files = (args.paths as string[]).map((path) =>
        file(path.split(/[\\/]/).pop()!, 1024 * 220),
      );
      const t = transfer(
        Math.floor(Math.random() * 10000) + 10,
        "you",
        files,
        chat.members
          .filter((m) => chat.members.length === 1 || m !== "you")
          .map((m) => receipt(m, "waiting")),
        0,
        chat.repo,
      );
      preview.transfers.push(t);
      value = t.key;
      break;
    }
    case "receive_files": {
      const t = preview.transfers.find((t) => t.key === args.key)!;
      t.deliveries.forEach((d) => {
        if (d.recipient === "you") d.status = "receiving";
      });
      setTimeout(() => {
        t.deliveries.forEach((d) => {
          if (d.recipient === "you") d.status = "received";
        });
        change();
      }, 2200);
      value = "Downloads / Cricket-preview";
      break;
    }
    case "retry_transfer": {
      const d = preview.transfers
        .find((t) => t.key === args.key)!
        .deliveries.find((d) => d.recipient === args.recipient)!;
      d.status = "waiting";
      d.attempt++;
      break;
    }
    case "request_retry":
      preview.transfers
        .find((t) => t.key === args.key)!
        .deliveries.filter((d) => d.recipient === "you")
        .forEach((d) => (d.status = "retry_requested"));
      break;
    case "disconnect":
      preview = {
        ...preview,
        user: null,
        connected: false,
        chats: [],
        transfers: [],
      };
      break;
    case "connect_cli":
    case "connect_token":
      throw new Error(
        "GitHub sign-in is available in the desktop app. This is a design preview.",
      );
    default:
      throw new Error(
        `This action is only available in the desktop app: ${command}`,
      );
  }
  if (command !== "snapshot") change();
  return value as T;
}
