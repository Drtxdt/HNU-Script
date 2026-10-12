"""Extract and test the ZIP from a Chinese path with development PATH removed.

This checks relocation/isolation on the current machine, not a clean Windows VM.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile
import runpy

ROOT = Path(__file__).resolve().parents[1]
VERSION = runpy.run_path(str(ROOT / "compiler_lexer_killer/__init__.py"))["__version__"]


def main():
    qa = ROOT / "build/qa"
    qa.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix="中文便携验证 ", dir=ROOT / "build"))
    with zipfile.ZipFile(ROOT / f"dist/Compiler-Lexer-Killer-{VERSION}-win64.zip") as archive:
        for name in archive.namelist():
            if not (staging / name).resolve().is_relative_to(staging.resolve()):
                raise RuntimeError("Unsafe archive path")
        archive.extractall(staging)
    folder = staging / "Compiler-Lexer-Killer"
    executable = folder / "Compiler-Lexer-Killer.exe"
    for scale in ("1", "1.5"):
        report = qa / f"portable-{scale}.json"
        report.unlink(missing_ok=True)
        env = os.environ.copy()
        for key in tuple(env):
            if key.upper().startswith(("PYTHON", "QT_", "QML")) or key.upper() == "GRAPHVIZ_DOT":
                del env[key]
        env["PATH"] = str(Path(env.get("SYSTEMROOT", "C:/Windows")) / "System32")
        env["LEXER_SMOKE_OUTPUT"] = str(report)
        env["CLK_DATA_DIR"] = str(staging / "user-data")
        env["QT_SCALE_FACTOR"] = scale
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        process = subprocess.run([str(executable), "--smoke-test"], cwd=folder, env=env,
                                 timeout=120, startupinfo=startup)
        if process.returncode or not report.exists():
            raise RuntimeError(f"Portable app failed: exit {process.returncode}, report {report}")
        data = json.loads(report.read_text(encoding="utf-8"))
        assert data["ok"] and data["frozen"], data
        assert Path(data["dot"]).is_relative_to(folder)
        assert Path(data["qt_module"]).is_relative_to(folder)
        assert all(Path(p).is_relative_to(folder) for p in data["python_paths"]), data["python_paths"]
        data.update(validation_environment="existing Windows host, isolated PATH; not a clean VM", display_scale=scale)
        report.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"PASS portable app at {scale}x DPI; bundled Python/Qt/Graphviz; Chinese relocation path")


if __name__ == "__main__":
    main()
