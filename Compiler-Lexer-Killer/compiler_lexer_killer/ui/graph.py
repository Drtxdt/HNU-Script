import math
from PySide6.QtCore import Qt, QPointF, QRectF, Signal
from PySide6.QtGui import QColor, QBrush, QPen, QPainter, QPainterPath, QPolygonF
from PySide6.QtWidgets import (QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton,
                               QGraphicsView, QGraphicsScene, QGraphicsEllipseItem)


class NodeItem(QGraphicsEllipseItem):
    def __init__(self, rect, number, callback):
        super().__init__(*rect)
        self.number, self.callback = number, callback
        self.setCursor(Qt.PointingHandCursor)

    def mousePressEvent(self, event):
        self.callback(self.number)
        super().mousePressEvent(event)


class Canvas(QGraphicsView):
    def wheelEvent(self, event):
        amount = 1.15 if event.angleDelta().y() > 0 else 1 / 1.15
        current = self.transform().m11()
        if 0.08 < current * amount < 5:
            self.scale(amount, amount)
        event.accept()


class GraphView(QWidget):
    state_clicked = Signal(int)

    def __init__(self, title):
        super().__init__()
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        row = QHBoxLayout()
        heading = QLabel(title)
        heading.setObjectName("sectionTitle")
        row.addWidget(heading)
        row.addStretch()
        fit = QPushButton("适应窗口")
        fit.clicked.connect(self.fit)
        row.addWidget(fit)
        layout.addLayout(row)
        self.scene = QGraphicsScene(self)
        self.view = Canvas(self.scene)
        self.view.setRenderHint(QPainter.Antialiasing)
        self.view.setDragMode(QGraphicsView.ScrollHandDrag)
        self.view.setTransformationAnchor(QGraphicsView.AnchorUnderMouse)
        self.view.setMinimumHeight(170)
        layout.addWidget(self.view)
        self.nodes, self.edges, self.markers = {}, {}, {}
        self.start_marker = None
        self.clear()

    def clear(self):
        self.scene.clear()
        self.nodes, self.edges, self.markers = {}, {}, {}
        self.start_marker = None
        self.scene.addText("生成后在这里查看自动机\n滚轮缩放 · 拖动画布 · 点击状态查看详情").setDefaultTextColor(QColor("#64748b"))

    def load(self, data):
        self.scene.clear()
        self.nodes, self.edges, self.markers = {}, {}, {}
        nodes, edges, start, positions, curves = data
        for index, (source, target, label) in enumerate(edges):
            points, lx, ly = curves[index]
            path = QPainterPath(QPointF(*points[0]))
            if (len(points) - 1) % 3 == 0:
                for i in range(1, len(points), 3):
                    path.cubicTo(QPointF(*points[i]), QPointF(*points[i+1]), QPointF(*points[i+2]))
            else:
                for point in points[1:]:
                    path.lineTo(QPointF(*point))
            item = self.scene.addPath(path, QPen(QColor("#94a3b8"), 1.5))
            px, py = points[-1]
            dx, dy = px - points[-2][0], py - points[-2][1]
            norm = math.hypot(dx, dy) or 1
            dx, dy = dx / norm, dy / norm
            tip = QPointF(px + dx * 7, py + dy * 7)
            arrow = self.scene.addPolygon(QPolygonF([tip, QPointF(px-dy*4, py+dx*4), QPointF(px+dy*4, py-dx*4)]),
                                          QPen(Qt.NoPen), QBrush(QColor("#94a3b8")))
            text = self.scene.addSimpleText(label if len(label) <= 36 else label[:33] + "…")
            bounds = text.boundingRect()
            text.setPos(lx-bounds.width()/2, ly-bounds.height()/2)
            text.setToolTip(label)
            background = self.scene.addRect(text.sceneBoundingRect().adjusted(-2,-1,2,1), QPen(Qt.NoPen), QBrush(QColor("#ffffff")))
            background.setZValue(1)
            text.setZValue(2)
            self.edges[index] = (item, arrow, text, background)
        for number, label, accept, tooltip in nodes:
            x, y, width, height = positions[number]
            item = NodeItem((x-width/2,y-height/2,width,height), number, self.state_clicked.emit)
            item.setZValue(3)
            item.setToolTip(tooltip)
            self.scene.addItem(item)
            text = self.scene.addSimpleText(label)
            text.setParentItem(item)
            text.setPos(x-text.boundingRect().width()/2,y-text.boundingRect().height()/2)
            text.setAcceptedMouseButtons(Qt.NoButton)
            ring = QGraphicsEllipseItem(x-width/2+5,y-height/2+5,width-10,height-10,item)
            ring.setPen(QPen(QColor("#059669"), 1.5))
            ring.setAcceptedMouseButtons(Qt.NoButton)
            ring.setVisible(accept)
            item.accept_ring = ring
            self.nodes[number] = item
            self.markers[number] = accept
        x, y, w, _ = positions[start]
        marker = self.scene.addSimpleText("开始 →")
        marker.setPos(x-w/2-marker.boundingRect().width()-7, y-marker.boundingRect().height()/2)
        self.start_marker = (start, marker)
        self.scene.setSceneRect(self.scene.itemsBoundingRect().adjusted(-20,-20,20,20))
        self.show_subset(set(self.nodes), set(self.edges))
        self.fit()

    def fit(self):
        bounds = QRectF()
        for item in self.scene.items():
            if item.isVisible():
                bounds = bounds.united(item.sceneBoundingRect())
        if not bounds.isEmpty():
            self.view.fitInView(bounds.adjusted(-20,-20,20,20), Qt.KeepAspectRatio)
            if self.view.transform().m11() > 1.4:
                self.view.resetTransform()
                self.view.scale(1.4, 1.4)

    def show_step(self, node_count, edge_count, active=(), highlighted=()):
        self.show_subset({n for n in self.nodes if n < node_count}, set(range(edge_count)), active, highlighted)

    def show_subset(self, visible_nodes, visible_edges, active=(), highlighted=(), accepts=None):
        active, highlighted = set(active), set(highlighted)
        for number, item in self.nodes.items():
            item.setVisible(number in visible_nodes)
            if accepts is not None:
                self.markers[number] = number in accepts
                item.accept_ring.setVisible(number in accepts)
            selected = number in active
            color = "#2563eb" if selected else "#059669" if self.markers[number] else "#64748b"
            item.setPen(QPen(QColor(color), 3 if selected else 1.8))
            item.setBrush(QBrush(QColor("#dbeafe" if selected else "#ecfdf5" if self.markers[number] else "#ffffff")))
        for index, (path, arrow, text, background) in self.edges.items():
            for item in (path, arrow, text, background):
                item.setVisible(index in visible_edges)
            color = QColor("#2563eb" if index in highlighted else "#94a3b8")
            path.setPen(QPen(color, 3 if index in highlighted else 1.5))
            arrow.setBrush(QBrush(color))
            text.setBrush(QBrush(QColor("#1d4ed8" if index in highlighted else "#475569")))
        if self.start_marker:
            self.start_marker[1].setVisible(self.start_marker[0] in visible_nodes)
