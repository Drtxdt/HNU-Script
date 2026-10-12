import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ.setdefault("CLK_TESTING", "1")
import copy
import json
import time
from pathlib import Path
import pytest
from PySide6.QtCore import Qt, QPoint, QPointF
from PySide6.QtGui import QFontDatabase
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication
from compiler_lexer_killer.ui.lab import LabWindow
from compiler_lexer_killer.ui.export import export_scene
from compiler_lexer_killer.core import compile, simulate, check_equivalence
from compiler_lexer_killer.core.manual import machine_from_document, document_from_machine
from compiler_lexer_killer.core.exercises import make_question, grade
from compiler_lexer_killer.project import save_project, load_project
from compiler_lexer_killer.exporting import write_report, write_data


@pytest.fixture(scope="module")
def app():
    app = QApplication.instance() or QApplication([])
    for name in ("msyh.ttc", "consola.ttf", "segoeui.ttf"):
        path = Path(os.environ.get("WINDIR", "C:/Windows"))/"Fonts"/name
        if path.exists():
            QFontDatabase.addApplicationFont(str(path))
    return app


@pytest.fixture
def window(app, tmp_path):
    os.environ["CLK_DATA_DIR"] = str(tmp_path)
    window = LabWindow()
    window.show()
    app.processEvents()
    yield window
    window.document_dirty = False
    window.close()
    app.processEvents()


def wait(app, predicate, timeout=20):
    deadline = time.monotonic()+timeout
    while time.monotonic() < deadline:
        app.processEvents()
        if predicate():
            return
        time.sleep(.01)
    raise AssertionError("asynchronous task timed out")


def test_full_lab_flow(window, app, tmp_path):
    window.build()
    wait(app, lambda: window.result is not None and not window.workers)
    window.run_simulation()
    assert window.simulation.accepted
    window.optimization.build()
    wait(app, lambda: window.optimization.data is not None)
    data = window.optimization.data
    assert check_equivalence(data["dfa"], data["min"].dfa).equivalent
    window.optimization.timeline.seek(len(data["min"].steps)-1)
    window.optimization.kind.setCurrentIndex(1)
    window.optimization.timeline.seek(len(data["steps"])-1)
    visible = {n for n,item in window.optimization.after.nodes.items() if item.isVisible()}
    assert visible == set(data["simple"].nfa.states)
    window.lexer_page.build()
    wait(app, lambda: window.lexer_page.compilation is not None)
    window.lexer_page.analyze()
    wait(app, lambda: window.lexer_page.run is not None)
    assert [t.rule for t in window.lexer_page.run.tokens] == ["IF", "ID", "INT", "ID"]
    window.copy_machine("regular")
    wait(app, lambda: not window.drawing.jobs.workers)
    window.drawing.build()
    wait(app, lambda: "drawing" in window.machines)
    window.run_simulation()
    assert window.simulation.accepted
    window.drawing.undo.undo()
    assert "drawing" not in window.machines
    path = tmp_path/"实验.clkproj"
    original = window.snapshot()
    save_project(path, original)
    window.apply_project(load_project(path))
    assert window.snapshot()["drawing"] == original["drawing"]
    assert window.snapshot()["rules"] == original["rules"]
    assert not window.machines


def test_mouse_edit_undo_export(window, app, tmp_path):
    editor = window.drawing
    window.tabs.setCurrentWidget(editor)
    app.processEvents()
    QTest.mouseDClick(editor.view.viewport(), Qt.LeftButton, pos=QPoint(180,140))
    app.processEvents()
    assert len(editor.document["nodes"]) == 1
    QTest.mouseDClick(editor.view.viewport(), Qt.LeftButton, pos=QPoint(380,190))
    app.processEvents()
    assert len(editor.document["nodes"]) == 2
    editor.tool.setCurrentIndex(1)
    for number in [0,1]:
        QTest.mouseClick(editor.view.viewport(), Qt.LeftButton, pos=editor.view.mapFromScene(editor.node_items[number].pos()))
        app.processEvents()
    assert len(editor.document["edges"]) == 1
    for _ in range(2):
        QTest.mouseClick(editor.view.viewport(), Qt.LeftButton, pos=editor.view.mapFromScene(editor.node_items[1].pos()))
        app.processEvents()
    assert editor.document["edges"][-1]["source"] == editor.document["edges"][-1]["target"] == 1
    editor.nodes_table.item(1,3).setCheckState(Qt.Checked)
    editor.tool.setCurrentIndex(0)
    old = copy.deepcopy(editor.document)
    point = editor.view.mapFromScene(editor.node_items[1].pos())
    QTest.mousePress(editor.view.viewport(), Qt.LeftButton, pos=point)
    QTest.mouseMove(editor.view.viewport(), point+QPoint(55,30), 40)
    QTest.mouseRelease(editor.view.viewport(), Qt.LeftButton, pos=point+QPoint(55,30))
    app.processEvents()
    assert editor.document != old
    moved = copy.deepcopy(editor.document)
    editor.undo.undo()
    assert editor.document == old
    editor.undo.redo()
    assert editor.document == moved
    machine = machine_from_document(editor.document)
    assert check_equivalence(machine, compile("a+").dfa).equivalent
    for suffix in ["svg", "png"]:
        path = tmp_path/f"中文 自环.{suffix}"
        export_scene(editor.scene, path)
        assert path.stat().st_size > 300
    editor.node_items[1].setSelected(True)
    editor.delete_selected()
    assert len(editor.document["nodes"]) == 1 and not editor.document["edges"]
    editor.undo.undo()
    assert editor.document == moved


def test_exercises_and_stale_work(window, app):
    page = window.exercises
    page.make()
    wait(app, lambda: page.question is not None)
    page.answer.setPlainText(json.dumps(page.question["expected"]))
    page.check()
    wait(app, lambda: page.feedback.text().startswith("✓"))
    question = make_question(8, kind="automaton")
    answer = document_from_machine(compile(question["source"], construction="simplified").nfa)
    assert grade(question, answer).correct
    page.answer.setPlainText("[]")
    page.check()
    wait(app, lambda: bool(page.mistakes))
    window.lexer_page.build()
    window.lexer_page.definitions.setPlainText("letter -> [a-z]")
    wait(app, lambda: not window.lexer_page.jobs.workers)
    assert window.lexer_page.compilation is None
    assert not window.lexer_page.run_button.isEnabled()


def test_export_data_report_and_recovery(window, app, tmp_path):
    window.build()
    wait(app, lambda: window.result is not None)
    window.nfa_timeline.seek(len(window.result.nfa_steps)-1)
    scene, owner = window.active_graph(True)
    export_scene(scene, tmp_path/"graph.svg")
    if owner:
        owner.deleteLater()
    write_data(tmp_path/"table.json", window.result.dfa)
    write_data(tmp_path/"table.csv", window.result.dfa)
    write_report(tmp_path/"report.md", window.snapshot(), window.result.dfa, image_name="graph.svg", steps=window.result.dfa_steps)
    assert "DFA" in (tmp_path/"report.md").read_text(encoding="utf-8")
    window.mark_dirty()
    window.save_recovery()
    assert load_project(window.recovery_path)["source"] == "(a|b)*abb"
