import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
import json
import time
from pathlib import Path
import pytest
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QFontDatabase
from compiler_lexer_killer.core import compile, Cancelled
from compiler_lexer_killer.core import CompileError
from compiler_lexer_killer.ui.app import MainWindow, PACKAGE
from compiler_lexer_killer.ui.layout import compute_layout


@pytest.fixture(scope="module")
def app():
    instance = QApplication.instance() or QApplication([])
    # Qt's offscreen Windows plugin does not enumerate system fonts.
    for name in ("msyh.ttc", "consola.ttf", "segoeui.ttf"):
        path = Path(os.environ.get("WINDIR", "C:/Windows")) / "Fonts" / name
        if path.exists():
            QFontDatabase.addApplicationFont(str(path))
    instance.setStyleSheet((PACKAGE / "ui/styles.qss").read_text(encoding="utf-8"))
    return instance


@pytest.fixture
def window(app):
    window = MainWindow()
    window.show()
    app.processEvents()
    yield window
    window.close()
    deadline = time.monotonic() + 5
    while window.workers and time.monotonic() < deadline:
        app.processEvents()
        time.sleep(0.01)
    assert not window.workers


def build_sync(window):
    result = compile(window.editor.toPlainText(), window.mode.currentData(), window.entry.currentText() or None)
    layouts = {kind: compute_layout(result, kind) for kind in ("nfa", "dfa")}
    window.built(window.generation, result, layouts)
    return result, layouts


def test_layout_preserves_parallel_edges_and_loops():
    result = compile("(a|b)*abb")
    for kind in ("nfa", "dfa"):
        nodes, edges, _, positions, curves = compute_layout(result, kind)
        assert len(positions) == len(nodes)
        assert len(curves) == len(edges)
        assert all(len(curve[0]) >= 2 for curve in curves.values())
    with pytest.raises(Cancelled):
        compute_layout(result, "nfa", lambda: True)


def test_replay_seek_and_invalidation(window, app):
    result, layouts = build_sync(window)
    last = len(result.dfa_steps)-1
    window.dfa_timeline.seek(last)
    expected = [[window.table.item(i,j+2).text() for j in range(len(result.dfa.alphabet))] for i in range(len(result.dfa.states))]
    window.dfa_timeline.seek(0)
    assert all(window.table.item(i,2).text() == "" for i in range(len(result.dfa.states)))
    window.dfa_timeline.seek(last)
    assert expected == [[window.table.item(i,j+2).text() for j in range(len(result.dfa.alphabet))] for i in range(len(result.dfa.states))]
    assert all(item.isVisible() for item in window.dfa_graph.nodes.values())
    window.nfa_timeline.seek(len(result.nfa_steps)-1)
    positions = {i: item.rect() for i,item in window.nfa_graph.nodes.items()}
    window.nfa_timeline.seek(0)
    assert positions == {i: item.rect() for i,item in window.nfa_graph.nodes.items()}
    window.run_simulation()
    window.sim_timeline.seek(len(window.simulation.steps)-1)
    assert window.simulation.accepted
    old_generation = window.generation
    window.dfa_timeline.toggle()
    window.editor.setPlainText("a+")
    assert not window.dfa_timeline.timer.isActive()
    assert window.result is None and window.simulation is None
    window.built(old_generation, result, layouts)
    assert window.result is None


def test_all_presets(window):
    for index, example in enumerate(window.examples):
        window.load_preset(index)
        result, _ = build_sync(window)
        window.run_simulation()
        assert window.simulation is not None
        assert result.entry == (result.definitions[-1] if result.definitions else None)


def test_reference_tree_selects_each_occurrence(window):
    window.mode.setCurrentIndex(1)
    window.editor.setPlainText("unit -> 'a'\npair -> unit unit")
    build_sync(window)
    root = window.ast_tree.topLevelItem(0)
    first, second = root.child(0).child(0), root.child(1).child(0)
    window.ast_clicked(first, 0)
    first_step = window.nfa_timeline.slider.value()
    window.ast_clicked(second, 0)
    assert window.nfa_timeline.slider.value() > first_step


def test_gui_error_location(window):
    window.editor.setPlainText("a|")
    try:
        compile("a|")
    except CompileError as error:
        window.failed(window.generation, error)
    assert "第 1 行" in window.feedback.text()
    assert window.editor.extraSelections()
    assert window.generate.isEnabled()


def test_worker_completion_and_cancel(window, app):
    window.build()
    deadline = time.monotonic() + 20
    while window.result is None and time.monotonic() < deadline:
        app.processEvents()
        time.sleep(0.01)
    assert window.result is not None, window.feedback.text()
    window.build()
    window.cancel_build()
    while window.workers and time.monotonic() < deadline:
        app.processEvents()
        time.sleep(0.01)
    assert window.result is None


def test_screenshots(window, app):
    output = Path(__file__).resolve().parents[1] / "build" / "qa"
    output.mkdir(parents=True, exist_ok=True)
    result, _ = build_sync(window)
    window.nfa_timeline.seek(len(result.nfa_steps)-1)
    window.dfa_timeline.seek(7)
    window.run_simulation()
    window.sim_timeline.seek(5)
    for index in range(3):
        window.tabs.setCurrentIndex(index)
        app.processEvents()
        assert window.grab().save(str(output / f"page-{index+1}.png"))
