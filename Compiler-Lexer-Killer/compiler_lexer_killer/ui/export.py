from pathlib import Path
from PySide6.QtCore import QRectF, QSize
from PySide6.QtGui import QPainter, QImage, QColor
from PySide6.QtSvg import QSvgGenerator


def export_scene(scene, path):
    """Render a scene on the GUI thread; no event processing during the snapshot."""
    path = Path(path)
    rect = QRectF()
    for item in scene.items():
        if item.isVisible():
            rect = rect.united(item.sceneBoundingRect())
    if rect.isEmpty():
        raise ValueError("没有可导出的图形")
    rect = rect.adjusted(-20,-20,20,20)
    scale = min(2, 8192/max(rect.width(), rect.height()))
    width, height = max(1, int(rect.width()*scale)), max(1, int(rect.height()*scale))
    if width*height > 24_000_000:
        scale *= (24_000_000/(width*height))**.5
        width, height = max(1,int(rect.width()*scale)), max(1,int(rect.height()*scale))
    if path.suffix.lower() == ".svg":
        device = QSvgGenerator()
        device.setFileName(str(path))
        device.setSize(QSize(width, height))
        device.setViewBox(QRectF(0,0,width,height))
        device.setTitle("Compiler Lexer Killer · 自动机")
    else:
        device = QImage(width, height, QImage.Format_ARGB32)
        device.fill(QColor("white"))
    painter = QPainter(device)
    if not painter.isActive():
        raise OSError("无法创建导出文件")
    try:
        painter.setRenderHint(QPainter.Antialiasing)
        scene.render(painter, QRectF(0,0,width,height), rect)
    finally:
        painter.end()
    if isinstance(device, QImage) and not device.save(str(path)):
        raise OSError("PNG 保存失败")
