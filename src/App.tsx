import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  ArrowDown,
  ArrowUpRight,
  Check,
  CheckCheck,
  ChevronRight,
  CircleHelp,
  Clock3,
  File,
  FileArchive,
  FileImage,
  FileText,
  Folder,
  Github,
  Laptop,
  LoaderCircle,
  LockKeyhole,
  Monitor,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Search,
  Send,
  Settings2,
  ShieldCheck,
  Sparkles,
  Terminal,
  Upload,
  Users,
  X,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { call, desktop } from "./bridge";
import {
  chatSummary,
  incomingFor,
  needsRetry,
  sameUser,
  sizeLabel,
  statusText,
  type Chat,
  type QueuedFile,
  type SharedFile,
  type Snapshot,
  type Transfer,
} from "./types";

function CricketMark({ small = false }: { small?: boolean }) {
  return (
    <svg
      aria-hidden="true"
      className={small ? "cricket-mark small" : "cricket-mark"}
      viewBox="0 0 64 64"
      fill="none"
    >
      <path d="M21 39c-4-12 5-21 14-18 7 2 9 12 2 17-4 3-11 4-16 1Z" />
      <path d="m34 22 8-9m-14 8-2-10M22 36l-9 9h11m9-9 12 9h7M25 40l-3 10m11-11 2 10" />
      <circle cx="35" cy="27" r="1" />
    </svg>
  );
}
function FileIcon({ file }: { file: SharedFile }) {
  const ext = file.name.split(".").pop()?.toLowerCase();
  const Icon = file.directory
    ? Folder
    : ["zip", "7z", "tar", "gz"].includes(ext ?? "")
      ? FileArchive
      : ["png", "jpg", "jpeg", "webp", "svg", "heic"].includes(ext ?? "")
        ? FileImage
        : ["pdf", "txt", "md", "docx"].includes(ext ?? "")
          ? FileText
          : File;
  return (
    <span
      className={`file-icon ${ext === "zip" ? "peach" : ext === "jpg" ? "lavender" : ""}`}
    >
      <Icon size={22} strokeWidth={1.5} />
    </span>
  );
}
function Avatar({
  chat,
  login,
  small = false,
}: {
  chat: Chat;
  login: string;
  small?: boolean;
}) {
  const personal = chat.members.length === 1;
  const group = chat.members.length > 2;
  return (
    <span
      aria-hidden="true"
      className={`avatar ${personal ? "device" : group ? "group" : "person"} ${small ? "avatar-small" : ""}`}
    >
      {personal ? (
        <Laptop size={small ? 19 : 22} />
      ) : group ? (
        <Users size={small ? 19 : 22} />
      ) : (
        chat.name
          .split(/\s+/)
          .slice(0, 2)
          .map((x) => x[0])
          .join("")
          .toUpperCase() || login[0]
      )}
    </span>
  );
}
function Modal({
  title,
  children,
  onClose,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    return () => ref.current?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      onCancel={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
      aria-label={title}
    >
      <div className="dialog-inner">
        <div className="dialog-heading">
          <h2>{title}</h2>
          <button
            className="icon-button"
            aria-label="Close dialog"
            onClick={onClose}
          >
            <X size={19} />
          </button>
        </div>
        {children}
      </div>
    </dialog>
  );
}
const platformName = (platform: string) =>
  ({ windows: "Windows", linux: "Linux", macos: "macOS" })[platform] ??
  platform;
const time = (timestamp: number) =>
  new Date(timestamp * 1000).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });

export default function App() {
  const [state, setState] = useState<Snapshot | null>(null);
  const [selected, setSelected] = useState("");
  const [search, setSearch] = useState("");
  const [queued, setQueued] = useState<QueuedFile[]>([]);
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [modal, setModal] = useState<"new" | "settings" | "about" | null>(null);
  const [newName, setNewName] = useState("");
  const [newMembers, setNewMembers] = useState("");
  const [importRepo, setImportRepo] = useState("");
  const [token, setToken] = useState("");
  const [authCode, setAuthCode] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);
  const history = useRef<HTMLDivElement>(null);
  const busyRef = useRef(false);
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const refresh = useCallback(async () => {
    try {
      const value = await call<Snapshot>("snapshot");
      setState(value);
      setSelected((old) =>
        value.chats.some((c) => c.repo === old)
          ? old
          : (value.chats[0]?.repo ?? ""),
      );
    } catch (e) {
      setError(String(e));
    }
  }, []);
  const run = useCallback(
    async (name: string, action: () => Promise<unknown>) => {
      if (busyRef.current) return;
      busyRef.current = true;
      setBusy(name);
      setError("");
      try {
        await action();
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        busyRef.current = false;
        setBusy("");
      }
    },
    [refresh],
  );
  useEffect(() => {
    void refresh();
    let disposed = false;
    let cleanup: (() => void) | undefined;
    if (desktop)
      void listen("cricket:changed", () => void refresh()).then((fn) => {
        if (disposed) fn();
        else cleanup = fn;
      });
    else {
      window.addEventListener("cricket-preview", refresh);
      cleanup = () => window.removeEventListener("cricket-preview", refresh);
    }
    const tick = setInterval(refresh, 15_000);
    return () => {
      disposed = true;
      cleanup?.();
      clearInterval(tick);
    };
  }, [refresh]);
  const addPaths = useCallback(async (paths: string[]) => {
    if (!selectedRef.current) {
      setError("Choose or create a chat before adding files.");
      return;
    }
    const files = await call<SharedFile[]>("inspect_files", { paths });
    setQueued((old) =>
      [...old, ...files.map((f, i) => ({ ...f, path: paths[i] }))].filter(
        (f, i, all) => all.findIndex((x) => x.path === f.path) === i,
      ),
    );
  }, []);
  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        setDragging(payload.type === "over" || payload.type === "enter");
        if (payload.type === "drop")
          void run("files", () => addPaths(payload.paths));
      })
      .then((fn) => {
        if (disposed) fn();
        else cleanup = fn;
      });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [addPaths, run]);
  useEffect(() => {
    setQueued([]);
    if (selected && state?.connected)
      void call("mark_seen", { repo: selected })
        .then(refresh)
        .catch((e) => setError(String(e)));
  }, [selected, state?.connected, refresh]);
  useEffect(() => {
    history.current?.scrollTo({
      top: history.current.scrollHeight,
      behavior: "smooth",
    });
  }, [selected, state?.transfers.length]);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(""), 8000);
    return () => clearTimeout(timer);
  }, [notice]);
  useEffect(() => {
    if (!authCode) return;
    const timer = setInterval(() => {
      void call<boolean>("poll_github_login")
        .then((done) => {
          if (done) {
            setAuthCode("");
            void refresh();
            void call("sync")
              .then(refresh)
              .catch((e) => setError(String(e)));
          }
        })
        .catch((e) => {
          setAuthCode("");
          setError(String(e));
        });
    }, 5000);
    return () => clearInterval(timer);
  }, [authCode, refresh]);
  const link = async (url: string) => {
    if (desktop) await openUrl(url);
    else window.open(url, "_blank", "noopener,noreferrer");
  };
  const choose = async (directory = false) => {
    if (!desktop) {
      fileInput.current?.click();
      return;
    }
    const paths = await open({
      multiple: true,
      directory,
      title: directory ? "Choose folders to send" : "Choose files to send",
    });
    if (paths) await addPaths(Array.isArray(paths) ? paths : [paths]);
  };
  const previewFiles = (files: FileList | null) => {
    const selection = files
      ? Array.from(files).map((f) => ({
          name: f.name,
          size: f.size,
          directory: false,
          path: f.name,
        }))
      : [];
    if (selection.length)
      setQueued((old) =>
        [...old, ...selection].filter(
          (f, i, a) => a.findIndex((x) => x.path === f.path) === i,
        ),
      );
  };
  const receive = async (t: Transfer) => {
    const directory = desktop
      ? await open({
          directory: true,
          multiple: false,
          title: "Where should Cricket save these files?",
        })
      : "Downloads";
    if (typeof directory !== "string") return;
    const saved = await call<string>("receive_files", {
      key: t.key,
      directory,
    });
    setNotice(`Receiving into ${saved}. You can keep using Cricket.`);
  };
  const send = () =>
    run("send", async () => {
      await call("send_files", {
        repo: selected,
        paths: queued.map((f) => f.path),
      });
      setQueued([]);
    });
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (
        (e.ctrlKey || e.metaKey) &&
        e.key === "Enter" &&
        queued.length &&
        !modal
      ) {
        e.preventDefault();
        void send();
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "k") {
        e.preventDefault();
        document.getElementById("chat-search")?.focus();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });
  if (!state)
    return (
      <div className="loading">
        <CricketMark />
        <p>Opening Cricket…</p>
        {error && <p role="alert">{error}</p>}
      </div>
    );
  const login = state.user?.login ?? "";
  const chat = state.chats.find((c) => c.repo === selected);
  const transfers = state.transfers
    .filter((t) => t.repo === selected)
    .sort((a, b) => a.created_at - b.created_at);
  const waiting = state.transfers.filter(
    (t) =>
      incomingFor(t, state) &&
      t.deliveries.some(
        (d) =>
          sameUser(d.recipient, login) &&
          ["waiting", "seen"].includes(d.status),
      ),
  ).length;
  const connect = (method: "connect_cli" | "connect_token") =>
    run("connect", async () => {
      await call(method, method === "connect_token" ? { token } : {});
      setToken("");
      await refresh();
      await call("sync");
    });
  const oauth = () =>
    run("connect", async () => {
      const code = await call<{ user_code: string }>("begin_github_login");
      setAuthCode(code.user_code);
      await link("https://github.com/login/device");
    });
  const accountContent = (
    <>
      <p className="muted">
        Connect your GitHub account to keep your chats and delivery receipts
        together.
      </p>
      {state.oauth_available && (
        <button
          className="primary wide"
          disabled={Boolean(busy)}
          onClick={() => void oauth()}
        >
          <Github size={18} /> Continue with GitHub
        </button>
      )}
      {authCode && (
        <div className="verification">
          <span>Enter this code on GitHub</span>
          <strong>{authCode}</strong>
          <small>Waiting for you to authorize Cricket…</small>
        </div>
      )}
      <button
        className={`${state.oauth_available ? "secondary" : "primary"} wide`}
        disabled={Boolean(busy)}
        onClick={() => void connect("connect_cli")}
      >
        {busy === "connect" ? (
          <LoaderCircle className="spin" size={18} />
        ) : (
          <Github size={18} />
        )}
        Use GitHub CLI sign-in
      </button>
      <p className="fine">
        Already signed into <code>gh</code>? Cricket can use that account.
      </p>
      <details className="token-details">
        <summary>Connect with an access token instead</summary>
        <label htmlFor="github-token">GitHub access token</label>
        <input
          id="github-token"
          type="password"
          autoComplete="off"
          placeholder="Paste your token"
          value={token}
          onChange={(e) => setToken(e.target.value)}
        />
        <p className="fine">
          A classic token with the <code>repo</code> scope supports creating
          private chats and inviting people. It grants broad repository access.
          Stored in your system credential vault.
        </p>
        <button
          className="secondary wide"
          disabled={!token || Boolean(busy)}
          onClick={() => void connect("connect_token")}
        >
          Connect account
        </button>
      </details>
    </>
  );
  return (
    <div className="app-shell">
      {!desktop && (
        <div className="preview-banner">
          <Sparkles size={13} /> Interactive preview <span>·</span> Transfers
          here are simulated. No files leave your browser.
        </div>
      )}
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">
            <CricketMark small />
          </span>
          <span>
            Cricket<span className="brand-dot">.</span>
          </span>
        </div>
        <div className="sidebar-heading">
          <span>YOUR CHATS</span>
          <button
            className="icon-button"
            aria-label="New chat"
            title="New chat"
            data-testid="new-chat"
            disabled={!state.connected}
            onClick={() => setModal("new")}
          >
            <Plus size={20} />
          </button>
        </div>
        <div className="search-field">
          <Search size={16} />
          <input
            id="chat-search"
            aria-label="Search chats"
            placeholder="Find a chat"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <kbd>{state.platform === "macos" ? "⌘ K" : "Ctrl K"}</kbd>
        </div>
        <nav className="chat-list" aria-label="Chats">
          {state.chats
            .filter((c) =>
              `${c.name} ${c.members.join(" ")}`
                .toLowerCase()
                .includes(search.toLowerCase()),
            )
            .map((c) => {
              const unread = state.transfers.some(
                (t) =>
                  t.repo === c.repo &&
                  incomingFor(t, state) &&
                  t.deliveries.some(
                    (d) =>
                      sameUser(d.recipient, login) &&
                      ["waiting", "seen"].includes(d.status),
                  ),
              );
              return (
                <button
                  className={`chat-button ${selected === c.repo ? "selected" : ""}`}
                  disabled={Boolean(busy)}
                  key={c.repo}
                  aria-current={selected === c.repo ? "page" : undefined}
                  data-testid={`chat-${c.repo}`}
                  onClick={() => {
                    if (queued.length && selected !== c.repo) {
                      setError(
                        "Send or remove the selected files before switching chats.",
                      );
                      return;
                    }
                    setSelected(c.repo);
                  }}
                >
                  <Avatar chat={c} login={login} />
                  <span className="chat-copy">
                    <span className="chat-name">{c.name}</span>
                    <span className="chat-preview">
                      {chatSummary(c, state)}
                    </span>
                  </span>
                  {unread && (
                    <span className="unread-dot" aria-label="Files waiting" />
                  )}
                </button>
              );
            })}
          {state.connected && !state.chats.length && (
            <p className="sidebar-empty">
              A familiar face.
              <br />A file to share.
              <br />
              Start with a new chat.
            </p>
          )}
          {search &&
            !state.chats.some((c) =>
              `${c.name} ${c.members.join(" ")}`
                .toLowerCase()
                .includes(search.toLowerCase()),
            ) && <p className="sidebar-empty">No chats found.</p>}
        </nav>
        {state.invitations.length > 0 && (
          <section className="invitations" aria-label="Chat invitations">
            <span className="eyebrow">INVITATIONS</span>
            {state.invitations.map((i) => (
              <button
                key={i.id}
                className="invitation"
                disabled={Boolean(busy)}
                onClick={() =>
                  void run("invite", () =>
                    call("accept_invitation", { invitation: i.id }),
                  )
                }
              >
                <Users size={16} />
                <span>{i.repository.full_name.split("/")[0]} invited you</span>
                <span>
                  Join <ChevronRight size={13} />
                </span>
              </button>
            ))}
          </section>
        )}
        <div className="sidebar-bottom">
          <div className="quiet-note">
            <ShieldCheck size={16} />
            <span>Small app. A little closer.</span>
          </div>
          <button
            className="account-button"
            aria-label="Account and settings"
            onClick={() => setModal("settings")}
          >
            <span className="account-avatar">
              {(state.user?.name ?? login ?? "C").slice(0, 1) || "C"}
            </span>
            <span>
              <strong>
                {state.user?.name || login || "Make yourself at home"}
              </strong>
              <small>
                {state.connected ? (
                  <>
                    <span className="online-dot" />
                    {platformName(state.platform)} · this device
                  </>
                ) : (
                  "Connect GitHub to start"
                )}
              </small>
            </span>
            <Settings2 size={17} />
          </button>
        </div>
      </aside>
      <main
        className="main-pane"
        onDragOver={(e) => {
          if (!desktop) {
            e.preventDefault();
            setDragging(true);
          }
        }}
        onDragLeave={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node))
            setDragging(false);
        }}
        onDrop={(e) => {
          if (!desktop) {
            e.preventDefault();
            setDragging(false);
            if (!chat) {
              setError("Choose a chat first.");
              return;
            }
            previewFiles(e.dataTransfer.files);
          }
        }}
      >
        {!state.connected ? (
          <section className="onboarding">
            <div className="welcome-mark">
              <CricketMark />
            </div>
            <span className="eyebrow">LESS FRICTION. MORE CONNECTION.</span>
            <h1>Files, a little closer.</h1>
            <p className="welcome-copy">
              From your laptop to your desktop.
              <br />
              From you to your favourite people.
            </p>
            <div className="connect-card">{accountContent}</div>
            <div className="welcome-features">
              <span>
                <LockKeyhole size={15} /> Encrypted file transfer
              </span>
              <span>
                <Monitor size={15} /> Windows, Linux & macOS
              </span>
            </div>
          </section>
        ) : !chat ? (
          <section className="onboarding empty-home">
            <div className="welcome-mark">
              <CricketMark />
            </div>
            <span className="eyebrow">HELLO, {login.toUpperCase()}</span>
            <h1>
              Good things are
              <br />
              better shared.
            </h1>
            <p className="welcome-copy">
              Start a chat with a friend, your people,
              <br />
              or just your other computer.
            </p>
            <button className="primary" onClick={() => setModal("new")}>
              <Plus size={18} /> Start your first chat
            </button>
            <p className="fine">
              Each chat gets its own private GitHub repository.
            </p>
          </section>
        ) : (
          <>
            <header className="chat-header">
              <Avatar chat={chat} login={login} small />
              <div className="header-copy">
                <h1>{chat.name}</h1>
                <p>
                  {chat.members.length === 1
                    ? "Your files, between your devices"
                    : chat.members.length > 2
                      ? `${chat.members.length} people · ${chat.members
                          .filter((m) => !sameUser(m, login))
                          .map((m) => `@${m}`)
                          .join(", ")}`
                      : `@${chat.members.find((m) => !sameUser(m, login))}`}
                  <span className="header-separator">·</span>
                  <LockKeyhole size={11} /> Private chat
                </p>
              </div>
              <button
                className="icon-button"
                disabled={Boolean(busy)}
                aria-label="Sync chats"
                title="Sync chats"
                onClick={() => void run("sync", () => call("sync"))}
              >
                <RefreshCw
                  size={18}
                  className={busy === "sync" ? "spin" : ""}
                />
              </button>
              <button
                className="icon-button"
                aria-label="Chat details"
                title="Chat details"
                onClick={() => setModal("about")}
              >
                <MoreHorizontal size={22} />
              </button>
            </header>
            <div
              className="history"
              ref={history}
              aria-label={`Transfer history with ${chat.name}`}
            >
              <div className="chat-intro">
                <span className="intro-icon">
                  <LockKeyhole size={17} />
                </span>
                <p>A little space for your shared things.</p>
                <span>
                  Files travel with croc. Only transfer details live on GitHub.
                </span>
              </div>
              {!transfers.length ? (
                <div className="empty-chat">
                  <span className="empty-orbit">
                    <Send size={31} strokeWidth={1.3} />
                    <span className="orbit-dot" />
                  </span>
                  <h2>Send something their way.</h2>
                  <p>Drop a file below to get things moving.</p>
                </div>
              ) : (
                transfers.map((t, index) => {
                  const mine = t.source_device === state.device_id;
                  const incoming = incomingFor(t, state);
                  const jobs = state.jobs.filter((j) => j.key === t.key);
                  const myDelivery = t.deliveries.find((d) =>
                    sameUser(d.recipient, login),
                  );
                  const date = new Date(t.created_at * 1000).toLocaleDateString(
                    [],
                    { month: "long", day: "numeric" },
                  );
                  const showDate =
                    index === 0 ||
                    new Date(
                      transfers[index - 1].created_at * 1000,
                    ).toDateString() !==
                      new Date(t.created_at * 1000).toDateString();
                  const allReceived = t.deliveries.every(
                    (d) => d.status === "received",
                  );
                  const retry = t.deliveries.some((d) => needsRetry(d.status));
                  return (
                    <div key={t.key}>
                      {showDate && (
                        <div className="date-divider">
                          <span>
                            {new Date(t.created_at * 1000).toDateString() ===
                            new Date().toDateString()
                              ? "Today"
                              : date}
                          </span>
                        </div>
                      )}
                      <article
                        className={`transfer ${mine ? "outgoing" : "incoming"}`}
                        aria-label={`${mine ? "You" : t.sender} shared ${t.files.length} items`}
                        data-testid={`transfer-${t.issue}`}
                      >
                        <div className="transfer-byline">
                          <span>
                            {mine
                              ? "You"
                              : sameUser(t.sender, login)
                                ? t.device_name
                                : `@${t.sender}`}
                          </span>
                          <time
                            dateTime={new Date(
                              t.created_at * 1000,
                            ).toISOString()}
                          >
                            {time(t.created_at)}
                          </time>
                        </div>
                        <div
                          className={`transfer-card ${retry ? "has-retry" : ""}`}
                        >
                          <div className="file-list">
                            {t.files.map((f, i) => (
                              <div className="file-row" key={`${f.name}-${i}`}>
                                <FileIcon file={f} />
                                <div>
                                  <strong title={f.name}>{f.name}</strong>
                                  <span>
                                    {f.directory ? "Folder" : sizeLabel(f.size)}
                                    <span className="file-kind-dot">·</span>
                                    {f.directory
                                      ? "All included files"
                                      : f.name.split(".").pop()?.toUpperCase() +
                                        " file"}
                                  </span>
                                </div>
                                {allReceived && (
                                  <Check size={15} className="file-check" />
                                )}
                              </div>
                            ))}
                          </div>
                          {incoming &&
                            myDelivery &&
                            ["waiting", "seen"].includes(myDelivery.status) && (
                              <div className="receive-area">
                                <button
                                  className="primary receive-button"
                                  data-testid={`receive-${t.issue}`}
                                  disabled={Boolean(busy)}
                                  onClick={() =>
                                    void run(`receive-${t.issue}`, () =>
                                      receive(t),
                                    )
                                  }
                                >
                                  <ArrowDown size={17} /> Receive{" "}
                                  {t.files.length === 1 ? "file" : "files"}
                                </button>
                                <span>
                                  {desktop
                                    ? "Choose where to save"
                                    : "Try the preview"}
                                </span>
                              </div>
                            )}
                          {incoming && myDelivery?.status === "receiving" && (
                            <div className="receiving">
                              <LoaderCircle size={15} className="spin" />{" "}
                              Receiving your files…
                            </div>
                          )}
                          {incoming &&
                            myDelivery &&
                            (needsRetry(myDelivery.status) ||
                              myDelivery.status === "sent") && (
                              <div className="retry-area">
                                <p>
                                  {myDelivery.status === "retry_requested"
                                    ? "The sender has your retry request."
                                    : "This transfer needs a fresh connection."}
                                </p>
                                <button
                                  className="text-button"
                                  disabled={
                                    Boolean(busy) ||
                                    myDelivery.status === "retry_requested"
                                  }
                                  onClick={() =>
                                    void run("retry-request", () =>
                                      call("request_retry", { key: t.key }),
                                    )
                                  }
                                >
                                  <RefreshCw size={14} />
                                  {myDelivery.status === "retry_requested"
                                    ? "Retry requested"
                                    : "Ask to resend"}
                                </button>
                              </div>
                            )}
                          {mine && t.deliveries.length > 1 && (
                            <div className="group-receipts">
                              {t.deliveries.map((d) => (
                                <div
                                  className="group-receipt"
                                  key={d.recipient}
                                >
                                  <span className="mini-avatar">
                                    {d.recipient[0].toUpperCase()}
                                  </span>
                                  <span>@{d.recipient}</span>
                                  <small
                                    className={
                                      needsRetry(d.status) ? "amber" : ""
                                    }
                                  >
                                    {statusText[d.status]}
                                  </small>
                                  {needsRetry(d.status) && t.can_retry && (
                                    <button
                                      className="text-button"
                                      disabled={
                                        Boolean(busy) ||
                                        jobs.some((j) =>
                                          sameUser(j.recipient, d.recipient),
                                        )
                                      }
                                      aria-label={`Retry for ${d.recipient}`}
                                      onClick={() =>
                                        void run("retry", () =>
                                          call("retry_transfer", {
                                            key: t.key,
                                            recipient: d.recipient,
                                          }),
                                        )
                                      }
                                    >
                                      Retry
                                    </button>
                                  )}
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                        <div
                          className={`transfer-receipt ${retry ? "amber" : allReceived ? "complete" : ""}`}
                        >
                          {allReceived ? (
                            <CheckCheck size={14} />
                          ) : retry ? (
                            <RefreshCw size={12} />
                          ) : (
                            <Clock3 size={12} />
                          )}
                          <span>
                            {t.deliveries.length > 1
                              ? `${t.deliveries.filter((d) => d.status === "received").length} of ${t.deliveries.length} received`
                              : statusText[t.deliveries[0].status]}
                          </span>
                          {mine &&
                            t.deliveries.length === 1 &&
                            retry &&
                            t.can_retry && (
                              <button
                                className="text-button"
                                disabled={Boolean(busy) || jobs.length > 0}
                                onClick={() =>
                                  void run("retry", () =>
                                    call("retry_transfer", {
                                      key: t.key,
                                      recipient: t.deliveries[0].recipient,
                                    }),
                                  )
                                }
                              >
                                Retry transfer
                              </button>
                            )}
                          {jobs.map((j) => (
                            <button
                              key={j.id}
                              className="text-button"
                              disabled={Boolean(busy)}
                              onClick={() =>
                                void run("cancel", () =>
                                  call("cancel_transfer", { jobId: j.id }),
                                )
                              }
                            >
                              Cancel
                              {jobs.length > 1 ? ` for @${j.recipient}` : ""}
                            </button>
                          ))}
                        </div>
                      </article>
                    </div>
                  );
                })
              )}
            </div>
            <footer className="composer">
              <div
                className={`drop-zone ${dragging ? "dragging" : ""} ${queued.length ? "has-files" : ""}`}
                data-testid="file-drop-zone"
              >
                {queued.length ? (
                  <div
                    className="queued-files"
                    aria-label="Files ready to send"
                  >
                    {queued.map((f) => (
                      <div className="queued-file" key={f.path}>
                        <FileIcon file={f} />
                        <span>
                          <strong>{f.name}</strong>
                          <small>
                            {f.directory ? "Folder" : sizeLabel(f.size)}
                          </small>
                        </span>
                        <button
                          className="icon-button"
                          aria-label={`Remove ${f.name}`}
                          onClick={() =>
                            setQueued((old) =>
                              old.filter((x) => x.path !== f.path),
                            )
                          }
                        >
                          <X size={15} />
                        </button>
                      </div>
                    ))}
                  </div>
                ) : (
                  <button
                    className="drop-prompt"
                    onClick={() => void run("files", () => choose())}
                  >
                    <span className="drop-symbol">
                      <Upload size={20} strokeWidth={1.5} />
                    </span>
                    <span>
                      <strong>
                        {dragging
                          ? "Let go. We’ll take it from here."
                          : "Drop something worth sharing."}
                      </strong>
                      <small>
                        Drag files here, or <span>browse files</span>
                      </small>
                    </span>
                  </button>
                )}
                <div className="composer-actions">
                  <div className="add-actions">
                    <button
                      className="add-file-button"
                      aria-label="Add files"
                      title="Add files"
                      disabled={Boolean(busy)}
                      onClick={() => void run("files", () => choose())}
                    >
                      <Plus size={18} />
                      <span>{queued.length ? "Add more" : "Files"}</span>
                    </button>
                    <button
                      className="add-file-button"
                      aria-label="Add folders"
                      title="Add folders"
                      disabled={Boolean(busy)}
                      onClick={() => void run("files", () => choose(true))}
                    >
                      <Folder size={16} />
                      <span>Folder</span>
                    </button>
                  </div>
                  <button
                    className="primary send-button"
                    data-testid="send-files"
                    disabled={
                      !queued.length || Boolean(busy) || !state.croc_version
                    }
                    onClick={() => void send()}
                  >
                    {busy === "send" ? (
                      <LoaderCircle size={16} className="spin" />
                    ) : (
                      <Send size={16} />
                    )}
                    Send
                    {queued.length > 0 && (
                      <span className="send-count">{queued.length}</span>
                    )}
                  </button>
                </div>
              </div>
              <div className="composer-footnote">
                <span>
                  <LockKeyhole size={11} /> Encrypted with croc
                </span>
                <span>Keep both devices online until received.</span>
              </div>
            </footer>
          </>
        )}
        {dragging && (
          <div className="drag-overlay">
            <Upload size={38} />
            <h2>Drop your files here</h2>
            <p>Ready to share with {chat?.name ?? "your people"}.</p>
          </div>
        )}
      </main>
      <input
        ref={fileInput}
        type="file"
        multiple
        hidden
        onChange={(e) => {
          previewFiles(e.target.files);
          e.target.value = "";
        }}
      />
      <div className="status-bar">
        <span>
          <span className={`status-dot ${state.error ? "warning" : ""}`} />
          {state.pending_receipts
            ? `${state.pending_receipts} receipt${state.pending_receipts === 1 ? "" : "s"} waiting to sync`
            : state.connected
              ? state.error
                ? "Sync needs attention"
                : "All caught up"
              : "Ready when you are"}
        </span>
        <span>
          {waiting
            ? `${waiting} transfer${waiting === 1 ? "" : "s"} waiting`
            : state.device_name}
          <span className="status-divider">/</span>Cricket 0.1
        </span>
      </div>
      {(error || state.error) && (
        <div role="alert" className="toast error-toast">
          <CircleHelp size={19} />
          <span>{error || state.error}</span>
          <button
            className="icon-button"
            aria-label="Dismiss message"
            onClick={() => {
              setError("");
              if (state.error) setState({ ...state, error: null });
            }}
          >
            <X size={16} />
          </button>
        </div>
      )}
      {notice && (
        <div role="status" className="toast">
          <Check size={18} />
          <span>{notice}</span>
          <button
            className="icon-button"
            aria-label="Dismiss notification"
            onClick={() => setNotice("")}
          >
            <X size={16} />
          </button>
        </div>
      )}
      {modal === "new" && (
        <Modal title="Make a little connection." onClose={() => setModal(null)}>
          <p className="muted">
            A friend, a group, or your other devices. Every chat has a private
            space on GitHub.
          </p>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void run("new", async () => {
                const c = await call<Chat>("create_chat", {
                  name: newName,
                  members: newMembers
                    .split(/[\s,]+/)
                    .map((s) => s.replace(/^@/, ""))
                    .filter(Boolean),
                });
                setSelected(c.repo);
                setModal(null);
                setNewName("");
                setNewMembers("");
              });
            }}
          >
            <label htmlFor="chat-name">Chat name</label>
            <input
              autoFocus
              id="chat-name"
              required
              maxLength={80}
              placeholder="e.g. Weekend crew"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
            />
            <label htmlFor="chat-members">
              GitHub usernames <span>optional</span>
            </label>
            <input
              id="chat-members"
              placeholder="maya, oliver, jules"
              value={newMembers}
              onChange={(e) => setNewMembers(e.target.value)}
            />
            <p className="fine">
              Separate usernames with commas. Leave this empty for a chat
              between your own devices. Friends accept an invitation in Cricket
              or on GitHub.
            </p>
            <button
              className="primary wide"
              type="submit"
              disabled={!newName.trim() || Boolean(busy)}
            >
              {busy === "new" ? (
                <LoaderCircle className="spin" size={17} />
              ) : (
                <Plus size={17} />
              )}{" "}
              Create private chat
            </button>
          </form>
          <details className="token-details">
            <summary>Already have a chat repository?</summary>
            <label htmlFor="import-repo">Repository</label>
            <input
              id="import-repo"
              placeholder="owner/cricket-…"
              value={importRepo}
              onChange={(e) => setImportRepo(e.target.value)}
            />
            <button
              className="secondary wide"
              disabled={!importRepo || Boolean(busy)}
              onClick={() =>
                void run("import", async () => {
                  const c = await call<Chat>("import_chat", {
                    repo: importRepo,
                  });
                  setSelected(c.repo);
                  setModal(null);
                  setImportRepo("");
                  await call("sync");
                })
              }
            >
              Join existing chat
            </button>
          </details>
        </Modal>
      )}
      {modal === "settings" && (
        <Modal title="Your little corner." onClose={() => setModal(null)}>
          {state.connected ? (
            <>
              <div className="settings-account">
                <Github size={24} />
                <div>
                  <strong>@{login}</strong>
                  <small>Connected to GitHub</small>
                </div>
                <span className="pill">Connected</span>
              </div>
              <div className="settings-row">
                <span>
                  <Laptop size={17} /> This device
                </span>
                <strong>{state.device_name}</strong>
              </div>
              <div className="settings-row">
                <span>
                  <Monitor size={17} /> Operating system
                </span>
                <strong>{platformName(state.platform)}</strong>
              </div>
              <div className="settings-row">
                <span>
                  <ShieldCheck size={17} /> Transfer engine
                </span>
                <strong>
                  {state.croc_version?.replace("croc version ", "") ??
                    "Not available"}
                </strong>
              </div>
              <div className="settings-note">
                <Terminal size={19} />
                <div>
                  <strong>Easy for you. Easy for your agent.</strong>
                  <p>
                    Use the labelled controls or Cricket’s authenticated local
                    command interface.
                  </p>
                  <button
                    className="text-button"
                    onClick={() =>
                      void link(
                        "https://github.com/eddieyzhan/Cricket/blob/main/docs/AGENT.md",
                      )
                    }
                  >
                    Agent guide <ArrowUpRight size={13} />
                  </button>
                </div>
              </div>
              <p className="fine">
                Closing the window keeps Cricket in your system tray to receive
                notifications. Use the tray menu to quit.
              </p>
              <button
                className="secondary wide"
                disabled={Boolean(busy) || state.jobs.length > 0}
                onClick={() =>
                  void run("disconnect", async () => {
                    await call("disconnect");
                    setModal(null);
                  })
                }
              >
                Disconnect GitHub
              </button>
            </>
          ) : (
            accountContent
          )}
        </Modal>
      )}
      {modal === "about" && chat && (
        <Modal title={chat.name} onClose={() => setModal(null)}>
          <div className="chat-detail-avatar">
            <Avatar chat={chat} login={login} />
          </div>
          <p className="muted">
            {chat.members.length === 1
              ? "Sign into the same GitHub account on another device. Your chat will appear automatically."
              : "One chat, one private repository. Each person gets their own transfer and delivery receipt."}
          </p>
          <div className="member-list">
            {chat.members.map((m) => (
              <div key={m}>
                <span className="mini-avatar">{m[0].toUpperCase()}</span>
                <strong>@{m}</strong>
                {sameUser(m, login) && <span className="pill">You</span>}
              </div>
            ))}
          </div>
          <button
            className="secondary wide"
            onClick={() => void link(`https://github.com/${chat.repo}`)}
          >
            <Github size={17} />
            Open chat on GitHub
            <ArrowUpRight size={14} />
          </button>
          <p className="fine">
            Chat members can see filenames, transfer codes, and receipts. Files
            themselves are never uploaded to GitHub. The sender stays online
            while you receive.
          </p>
        </Modal>
      )}
    </div>
  );
}
