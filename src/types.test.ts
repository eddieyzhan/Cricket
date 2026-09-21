import { describe, it, expect } from "vitest";
import {
  incomingFor,
  needsRetry,
  sameUser,
  sizeLabel,
  type Snapshot,
  type Transfer,
} from "./types";
describe("transfer presentation", () => {
  it("distinguishes another device on the same account from this sender", () => {
    const state = { device_id: "laptop", user: { login: "alice" } } as Snapshot;
    const transfer = {
      source_device: "desktop",
      deliveries: [{ recipient: "alice" }],
    } as Transfer;
    expect(incomingFor(transfer, state)).toBe(true);
    expect(incomingFor({ ...transfer, source_device: "laptop" }, state)).toBe(
      false,
    );
  });
  it("matches GitHub logins case insensitively without treating missing users as equal", () => {
    expect(sameUser("Alice", "ALICE")).toBe(true);
    expect(sameUser(undefined, undefined)).toBe(false);
  });
  it("does not offer retries for a delivered or currently active transfer", () => {
    expect(needsRetry("received")).toBe(false);
    expect(needsRetry("receiving")).toBe(false);
    expect(needsRetry("expired")).toBe(true);
    expect(needsRetry("retry_requested")).toBe(true);
  });
  it("formats empty and large files without invalid units", () => {
    expect(sizeLabel(0)).toBe("0 B");
    expect(sizeLabel(1024)).toBe("1.0 KB");
    expect(sizeLabel(1024 ** 4)).toBe("1024.0 GB");
  });
});
