from PySide6.QtCore import Qt, QTimer, Signal
from PySide6.QtWidgets import QWidget, QHBoxLayout, QPushButton, QSlider, QLabel, QComboBox


class Timeline(QWidget):
    changed = Signal(int)

    def __init__(self):
        super().__init__()
        row = QHBoxLayout(self)
        row.setContentsMargins(0, 6, 0, 6)
        self.reset = QPushButton("重置")
        self.previous = QPushButton("上一步")
        self.play = QPushButton("播放")
        self.next = QPushButton("下一步")
        self.slider = QSlider(Qt.Horizontal)
        self.counter = QLabel("0 / 0")
        self.speed = QComboBox()
        for label, interval in [("0.5×", 1600), ("1×", 800), ("2×", 400), ("4×", 200)]:
            self.speed.addItem(label, interval)
        self.speed.setCurrentIndex(1)
        for widget in (self.reset, self.previous, self.play, self.next, self.slider, self.counter, self.speed):
            row.addWidget(widget)
        self.timer = QTimer(self)
        self.timer.setInterval(800)
        self.timer.timeout.connect(self.advance)
        self.reset.clicked.connect(lambda: self.seek(0))
        self.previous.clicked.connect(lambda: self.seek(self.slider.value()-1))
        self.next.clicked.connect(lambda: self.seek(self.slider.value()+1))
        self.play.clicked.connect(self.toggle)
        self.speed.currentIndexChanged.connect(lambda: self.timer.setInterval(self.speed.currentData()))
        self.slider.valueChanged.connect(self._changed)
        self.count = 0
        self.set_steps(0)

    def set_steps(self, count):
        self.stop()
        self.count = count
        self.slider.blockSignals(True)
        self.slider.setRange(0, max(count-1, 0))
        self.slider.setValue(0)
        self.slider.blockSignals(False)
        self.setEnabled(bool(count))
        self._changed(0)

    def _changed(self, value):
        self.counter.setText(f"{value+1 if self.count else 0} / {self.count}")
        self.previous.setEnabled(value > 0)
        self.next.setEnabled(value < self.count - 1)
        if value == self.count - 1:
            self.stop()
        self.changed.emit(value)

    def seek(self, value):
        self.stop()
        self.slider.setValue(value)

    def advance(self):
        if self.slider.value() >= self.count - 1:
            self.stop()
        else:
            self.slider.setValue(self.slider.value()+1)

    def toggle(self):
        if self.timer.isActive():
            self.stop()
        else:
            if self.slider.value() == self.count-1:
                self.slider.setValue(0)
            self.timer.start()
            self.play.setText("暂停")

    def stop(self):
        self.timer.stop()
        self.play.setText("播放")
