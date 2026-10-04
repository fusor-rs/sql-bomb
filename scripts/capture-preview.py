import errno
import fcntl
import os
from pathlib import Path
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time


ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / "target/debug/examples/site-preview"
OUTPUT = ROOT / "apps/landing/public/terminal"
COLUMNS = 110
ROWS = 28
TIMEOUT = 20
# Ratatui ends each draw with these resets; the sample has no hyperlink draw groups.
FRAME_END = b"\x1b[39m\x1b[49m\x1b[0m"
QUERY = """SELECT city, country, distance_km
FROM destinations
ORDER BY distance_km;"""


def render(output, output_format):
    return subprocess.run(
        [BINARY, output_format, str(COLUMNS), str(ROWS)],
        input=output,
        capture_output=True,
        check=True,
    ).stdout


def capture(terminal, output, name, expected):
    deadline = time.monotonic() + TIMEOUT
    while True:
        ready, _, _ = select.select([terminal], [], [], max(0, deadline - time.monotonic()))
        if not ready:
            raise TimeoutError(f"Waiting for {name}:\n{render(output, '--text').decode()}")
        chunk = os.read(terminal, 65536)
        if not chunk:
            raise RuntimeError(f"Native app exited while capturing {name}")
        output.extend(chunk)
        boundary = output.rfind(FRAME_END)
        if boundary < 0:
            continue
        frame = output[:boundary + len(FRAME_END)]
        text = render(frame, "--text").decode()
        if all(value in text for value in expected):
            (OUTPUT / f"{name}.svg").write_bytes(render(frame, "--svg"))
            print(f"Captured {name}")
            return


def walkthrough(terminal):
    output = bytearray()
    capture(terminal, output, "welcome", ["Ready", "Ctrl+C quit"])
    os.write(terminal, f"\x1b[200~{QUERY}\x1b[201~\x12".encode())
    capture(terminal, output, "results", ["Complete", "Row 1 / 6", "Tokyo"])
    os.write(terminal, b"\x0c\r")
    capture(terminal, output, "inspector", ["Row 1", "Utf8", "Scroll to read"])
    os.write(terminal, b"\x1b")
    capture(terminal, output, "results", ["Complete", "Row 1 / 6", "Arrows select"])
    os.write(terminal, b"\x13\x10")
    capture(terminal, output, "library", ["Query library", "›★ SELECT", "Menu for more actions"])
    os.write(terminal, b"\x0b")
    capture(terminal, output, "menu", ["Find an action", "Browse results", "Enter open"])
    os.write(terminal, b"\x03")


def main():
    subprocess.run(
        ["cargo", "build", "--example", "site-preview", "--locked", "--offline"],
        cwd=ROOT,
        check=True,
    )
    OUTPUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sql-bomb-preview-") as directory:
        child, terminal = os.forkpty()
        if child == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLUMNS, 0, 0))
            environment = {name: value for name, value in os.environ.items() if name != "NO_COLOR"}
            environment.update(TERM="xterm-256color", COLORTERM="truecolor")
            os.execve(BINARY, [str(BINARY), directory], environment)
        try:
            walkthrough(terminal)
            drain(terminal)
        except BaseException:
            os.kill(child, signal.SIGTERM)
            os.waitpid(child, 0)
            raise
        finally:
            os.close(terminal)
        _, status = os.waitpid(child, 0)
        if status != 0:
            raise RuntimeError(f"Native app exited with status {status}")


def drain(terminal):
    deadline = time.monotonic() + TIMEOUT
    while True:
        ready, _, _ = select.select([terminal], [], [], max(0, deadline - time.monotonic()))
        if not ready:
            raise TimeoutError("Native app did not close after Ctrl+C")
        try:
            if not os.read(terminal, 65536):
                return
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            # Linux PTYs report EIO when the slave closes.
            return


if __name__ == "__main__":
    main()
