import threading
from PySide6.QtCore import QObject, QThread, Signal
from ..core.model import Cancelled


class Worker(QThread):
    ready = Signal(int, object)
    failed = Signal(int, object)

    def __init__(self, revision, function, parent=None):
        super().__init__(parent)
        self.revision, self.function = revision, function
        self.cancel_event = threading.Event()

    def run(self):
        try:
            result = self.function(self.cancel_event.is_set)
            if not self.cancel_event.is_set():
                self.ready.emit(self.revision, result)
        except Cancelled:
            pass
        except Exception as exc:
            if not self.cancel_event.is_set():
                self.failed.emit(self.revision, exc)


class Jobs(QObject):
    result = Signal(object)
    error = Signal(object)
    busy = Signal(bool)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.revision, self.workers = 0, []

    def invalidate(self):
        self.revision += 1
        for worker in self.workers:
            worker.cancel_event.set()
        self.busy.emit(False)

    def start(self, function):
        self.invalidate()
        worker = Worker(self.revision, function, self)
        self.workers.append(worker)
        worker.ready.connect(self.deliver)
        worker.failed.connect(self.fail)
        worker.finished.connect(self.collect)
        self.busy.emit(True)
        worker.start()

    def deliver(self, revision, value):
        if revision == self.revision:
            self.busy.emit(False)
            self.result.emit(value)

    def fail(self, revision, exc):
        if revision == self.revision:
            self.busy.emit(False)
            self.error.emit(exc)

    def collect(self):
        for worker in self.workers[:]:
            if worker.isFinished():
                self.workers.remove(worker)
                worker.deleteLater()

    def shutdown(self):
        self.invalidate()
        for worker in self.workers:
            worker.wait()
        self.collect()
