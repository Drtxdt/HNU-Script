"""Opt-in packaged application self-test, including Qt event-loop and Graphviz."""
import json
import os
import sys
import time
import traceback
import PySide6.QtCore
from pathlib import Path
from PySide6.QtCore import QTimer
from ..core import compile, simulate
from .layout import find_dot


def attach_smoke(app, window):
    output = Path(os.environ.get("LEXER_SMOKE_OUTPUT", "smoke-result.json"))
    started = time.monotonic()
    timer = QTimer(window)
    timer.setInterval(100)
    report = {"frozen": bool(getattr(sys, "frozen", False)), "executable": sys.executable,
              "qt_module": PySide6.QtCore.__file__, "python_paths": sys.path}

    def finish(code, error=None):
        timer.stop()
        report.update(ok=code == 0, elapsed_seconds=round(time.monotonic()-started, 2))
        if error:
            report["error"] = error
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        for worker in window.workers:
            worker.cancel_event.set()
        window.close()
        app.exit(code)

    def check():
        try:
            if time.monotonic()-started > 35:
                finish(1, "Build timeout: " + window.feedback.text())
                return
            if window.result is None:
                if not window.workers:
                    finish(1, window.feedback.text())
                return
            if window.workers:
                return
            result = window.result
            for text in ("abb", "aabb", "abababb"):
                assert simulate(result.dfa, text).accepted
            for text in ("", "ab", "abba"):
                assert not simulate(result.dfa, text).accepted
            window.nfa_timeline.seek(len(result.nfa_steps)-1)
            window.dfa_timeline.seek(len(result.dfa_steps)-1)
            window.dfa_timeline.seek(0)
            window.dfa_timeline.seek(len(result.dfa_steps)-1)
            window.run_simulation()
            window.sim_timeline.seek(len(window.simulation.steps)-1)
            assert window.simulation.accepted
            window.tabs.setCurrentIndex(2)
            app.processEvents()
            assert window.grab().save(str(output.with_suffix(".png")))
            named = compile("letter → [A-Za-z]\nid → letter+", "definitions")
            assert simulate(named.dfa, "initial").accepted
            report.update(dot=find_dot(), nfa_states=len(result.nfa.states), dfa_states=len(result.dfa.states),
                          presets=len(window.examples), assertions="matching, rejection, named definitions, replay, simulation, screenshot")
            finish(0)
        except Exception:
            finish(1, traceback.format_exc())

    timer.timeout.connect(check)
    QTimer.singleShot(0, window.build)
    timer.start()
