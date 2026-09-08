import { expect, it, vi } from "vitest";
import { writeLog } from "../lib/terminal-log";

it("continues rendering after the retained log reaches its cap, including repeated output", () => {
  const session = { term: { write: vi.fn(), reset: vi.fn() }, written: 0 };
  writeLog(session, "x".repeat(200000), 200000);
  writeLog(session, "x".repeat(200000), 200100);
  expect(session.term.write).toHaveBeenLastCalledWith("x".repeat(100), undefined);
  writeLog(session, "x".repeat(199997) + "end", 200103);
  expect(session.term.write).toHaveBeenLastCalledWith("end", undefined);
  expect(session.term.reset).not.toHaveBeenCalled();
});

it("replays the retained tail after a gap and resets when output is cleared", () => {
  const session = { term: { write: vi.fn(), reset: vi.fn() }, written: 10 };
  writeLog(session, "tail", 300000);
  expect(session.term.write).toHaveBeenLastCalledWith("tail", undefined);
  writeLog(session, "", 0);
  expect(session.term.reset).toHaveBeenCalledTimes(2);
});
