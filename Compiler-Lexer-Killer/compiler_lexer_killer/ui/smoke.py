"""Opt-in native Qt acceptance: workers, editing, grading, persistence, exports."""
import json
import os
import sys
import time
import traceback
from pathlib import Path
import PySide6.QtCore
from PySide6.QtCore import QTimer, Qt, QPointF
from PySide6.QtTest import QTest
from ..core import compile, simulate, check_equivalence
from ..core.manual import document_from_machine
from ..project import save_project, load_project
from ..exporting import write_report, write_data
from .layout import find_dot
from .export import export_scene
from .. import __version__


def attach_smoke(app, window):
    output = Path(os.environ.get('LEXER_SMOKE_OUTPUT', 'smoke-result.json'))
    started = time.monotonic()
    timer = QTimer(window)
    timer.setInterval(100)
    report = {'version': __version__, 'frozen': bool(getattr(sys, 'frozen', False)), 'executable': sys.executable,
              'qt_module': PySide6.QtCore.__file__, 'python_paths': sys.path}
    phase = 0
    finished = False
    folder = output.parent / (output.stem + '-artifacts')
    folder.mkdir(parents=True, exist_ok=True)
    screen = window.screen().availableGeometry()
    window.resize(min(1420, screen.width()-40), min(900, screen.height()-60))
    report['screen_available'] = [screen.width(), screen.height()]

    def finish(code, error=None):
        nonlocal finished
        if finished:
            return
        finished = True
        timer.stop()
        report.update(ok=code == 0, elapsed_seconds=round(time.monotonic()-started, 2))
        if error:
            report['error'] = error
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
        window.document_dirty = False
        window.close()
        app.exit(code)

    def shot(name, index):
        window.tabs.setCurrentIndex(index)
        app.processEvents()
        if index == 3:
            window.optimization.before.fit()
            window.optimization.after.fit()
        if index == 4:
            window.lexer_page.graph.fit()
        assert window.grab().save(str(folder / (name + '.png')))

    def check():
        nonlocal phase
        try:
            if time.monotonic()-started > 100:
                finish(1, f'Phase {phase} timeout; ' + window.feedback.text() + '; ' + window.lexer_page.message.text())
                return
            if phase == 0:
                if window.result is None or window.workers:
                    return
                for text in ('abb', 'aabb', 'abababb'):
                    assert simulate(window.result.dfa, text).accepted
                for text in ('', 'ab', 'abba'):
                    assert not simulate(window.result.dfa, text).accepted
                window.nfa_timeline.seek(len(window.result.nfa_steps)-1)
                window.dfa_timeline.seek(len(window.result.dfa_steps)-1)
                window.run_simulation()
                window.sim_timeline.seek(len(window.simulation.steps)-1)
                assert window.simulation.accepted
                shot('01-thompson', 0)
                shot('02-subset', 1)
                shot('03-simulation', 2)
                phase = 1
                window.optimization.build()
            elif phase == 1:
                if window.optimization.data is None:
                    return
                data = window.optimization.data
                assert check_equivalence(data['dfa'], data['min'].dfa).equivalent
                window.optimization.timeline.seek(len(data['min'].steps)-1)
                shot('04-minimize', 3)
                window.optimization.kind.setCurrentIndex(1)
                window.optimization.timeline.seek(len(data['steps'])-1)
                shot('04-simplify', 3)
                window.lexer_page.load_preset(2)
                phase = 2
                window.lexer_page.build()
            elif phase == 2:
                if window.lexer_page.compilation is None:
                    return
                phase = 3
                window.lexer_page.analyze()
            elif phase == 3:
                if window.lexer_page.run is None:
                    return
                run = window.lexer_page.run
                assert not run.errors and run.complete
                assert [(t.rule, t.text) for t in run.tokens[:6]] == [('IF','if'), ('LPAREN','('), ('ID','initial'), ('LE','<='), ('NUMBER','10'), ('RPAREN',')')]
                shot('05-tokens', 4)
                window.lexer_page.view_tabs.setCurrentWidget(window.lexer_page.graph)
                shot('05-lexer-graph', 4)
                editor = window.drawing
                window.tabs.setCurrentWidget(editor)
                editor.add_node(QPointF(0,0))
                editor.add_node(QPointF(180,0))
                editor.nodes_table.item(1,3).setCheckState(Qt.Checked)
                editor.fit()
                editor.tool.setCurrentIndex(1)
                for number in (0,1,1,1):
                    QTest.mouseClick(editor.view.viewport(), Qt.LeftButton, pos=editor.view.mapFromScene(editor.node_items[number].pos()))
                    app.processEvents()
                assert len(editor.document['edges']) == 2
                editor.tool.setCurrentIndex(0)
                from ..core.manual import machine_from_document
                assert check_equivalence(machine_from_document(editor.document), compile('a+').dfa).equivalent
                editor.undo.undo()
                assert len(editor.document['edges']) == 1
                editor.undo.redo()
                assert len(editor.document['edges']) == 2
                shot('06-drawing', 5)
                export_scene(editor.scene, folder/'手绘 自动机.svg')
                export_scene(editor.scene, folder/'手绘 自动机.png')
                phase = 4
                editor.build()
            elif phase == 4:
                if 'drawing' not in window.machines:
                    return
                window.input.setPlainText('aaa')
                window.run_simulation()
                assert window.simulation.accepted
                window.exercises.kind.setCurrentIndex(0)
                phase = 5
                window.exercises.make()
            elif phase == 5:
                if window.exercises.question is None:
                    return
                window.exercises.answer.setPlainText(json.dumps(window.exercises.question['expected']))
                phase = 6
                window.exercises.check()
            elif phase == 6:
                if not window.exercises.feedback.text().startswith('✓'):
                    return
                shot('07-exercises', 6)
                snapshot = window.snapshot()
                save_project(folder/'中文 工程.clkproj', snapshot)
                assert load_project(folder/'中文 工程.clkproj') == snapshot
                write_data(folder/'tokens.json', window.lexer_page.dfa, window.lexer_page.run)
                write_data(folder/'tokens.csv', window.lexer_page.dfa, window.lexer_page.run)
                export_scene(window.lexer_page.graph.scene, folder/'词法 DFA.svg')
                write_report(folder/'实验报告.md', snapshot, window.lexer_page.dfa, window.lexer_page.run,
                             image_name='词法 DFA.svg', note='便携自检：课件 8–10、136–145、155 页。',
                             context='lexer', strategy='联合 NFA、DFA、最长匹配与规则优先级')
                report.update(dot=find_dot(), nfa_states=len(window.result.nfa.states), dfa_states=len(window.result.dfa.states),
                              token_count=len(window.lexer_page.run.tokens), assertions='construction, optimization, lexer, native mouse editing, undo/redo, manual subset, simulation, grading, project, SVG/PNG, CSV/JSON/Markdown',
                              artifact_directory=str(folder))
                window.apply_project(load_project(folder/'中文 工程.clkproj'))
                assert window.snapshot()['drawing'] == snapshot['drawing']
                assert window.exercises.answer.toPlainText() == snapshot['exercise']['answer']
                finish(0)
        except Exception:
            finish(1, traceback.format_exc())

    timer.timeout.connect(check)
    QTimer.singleShot(0, window.build)
    timer.start()
