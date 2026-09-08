type LogSession = {
  term: { write: (data: string, callback?: () => void) => void; reset: () => void };
  written: number;
};

export function writeLog(session: LogSession, log: string, end: number, onWritten?: () => void) {
  const start = end - log.length;
  if (session.written < start || session.written > end) {
    session.term.reset();
    session.written = start;
  }
  if (end > session.written) {
    session.term.write(log.slice(session.written - start), onWritten);
    session.written = end;
  } else {
    onWritten?.();
  }
}
