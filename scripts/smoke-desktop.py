#!/usr/bin/env python3
"""Exercise the actual distributable, using only temporary synthetic projects."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time


def has_window(pid):
    if os.name != "nt":
        # This runs inside a dedicated Xvfb server in CI. AppImage's launcher
        # may fork, so find the app's visible window rather than its wrapper PID.
        return subprocess.run(
            ["xdotool", "search", "--onlyvisible", "--name", "^Plassi$"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        ).returncode == 0
    from ctypes import wintypes
    user32 = ctypes.windll.user32
    visible = []
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    @callback_type
    def visit(window, _):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(window, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(window):
            visible.append(window)
        return True

    user32.EnumWindows(visit, 0)
    return bool(visible)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("--gui", action="store_true")
    parser.add_argument("--log", type=Path, help="Keep the GUI log outside the temporary project")
    args = parser.parse_args()
    executable = args.executable.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="plassi-smoke-") as directory:
        root = Path(directory)
        env = dict(os.environ, PLASSI_HOME=str(root / "app-data"))

        def run(*arguments):
            result = subprocess.run(
                [str(executable), *map(str, arguments)], cwd=root, env=env,
                capture_output=True, text=True, encoding="utf-8", timeout=60,
            )
            if result.returncode:
                raise RuntimeError(f"{arguments}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
            return result.stdout

        schema = json.loads(run("schema"))
        assert "attendees" in schema["properties"], "schema output is missing the project fields"
        source = root / "people.tsv"
        source.write_text("Name\tOrganization\nX\tABC\nY\tXYZ\n", encoding="utf-8")
        project = root / "project.json"
        run("import", source, project)
        people = json.loads(project.read_text(encoding="utf-8"))["attendees"]
        assert [person["name"] for person in people] == ["X", "Y"], people
        run("validate", project)
        print("Packaged CLI: schema, spreadsheet import and project validation passed.", flush=True)

        if args.gui:
            log = args.log.resolve() if args.log else root / "gui.log"
            with log.open("w", encoding="utf-8") as output:
                process = subprocess.Popen([str(executable), "gui"], cwd=root, env=env,
                                           stdout=output, stderr=output,
                                           start_new_session=os.name != "nt")
                try:
                    deadline = time.monotonic() + 30
                    while time.monotonic() < deadline:
                        if process.poll() is not None:
                            raise RuntimeError(f"GUI exited with {process.returncode}: {log.read_text(encoding='utf-8')}")
                        if has_window(process.pid):
                            time.sleep(2)
                            if process.poll() is not None:
                                raise RuntimeError(f"GUI exited with {process.returncode} (0x{process.returncode & 0xffffffff:08x}) immediately after opening its window: {log.read_text(encoding='utf-8')}")
                            print("Packaged GUI: visible native window opened and stayed running.", flush=True)
                            break
                        time.sleep(0.25)
                    else:
                        raise RuntimeError(f"No visible GUI window within 30 seconds: {log.read_text(encoding='utf-8')}")
                finally:
                    if os.name != "nt":
                        # AppImage launchers can fork; close the whole test's
                        # process group so no window or extracted image survives.
                        try:
                            os.killpg(process.pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                        process.wait(timeout=15)
                    elif process.poll() is None:
                        process.terminate()
                        process.wait(timeout=15)


if __name__ == "__main__":
    main()
