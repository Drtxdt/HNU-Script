"""Reproducible Windows one-folder build; downloads handled by prepare_graphviz.py."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import runpy
import urllib.request
import zipfile
import pefile
import PySide6

ROOT = Path(__file__).resolve().parents[1]
NAME = "Compiler-Lexer-Killer"
VERSION = runpy.run_path(str(ROOT / "compiler_lexer_killer/__init__.py"))["__version__"]


def graphviz_runtime():
    source = ROOT / "vendor/graphviz/bin"
    if not (source / "dot.exe").exists():
        subprocess.run([sys.executable, str(ROOT / "tools/prepare_graphviz.py")], check=True)
    target = ROOT / "build/graphviz/bin"
    target.mkdir(parents=True, exist_ok=True)
    available = {p.name.lower(): p for p in source.iterdir() if p.is_file()}
    pending = ["dot.exe", "gvplugin_core.dll", "gvplugin_dot_layout.dll"]
    copied = set()
    while pending:
        name = pending.pop().lower()
        if name in copied:
            continue
        path = available[name]
        shutil.copy2(path, target / path.name)
        copied.add(name)
        with pefile.PE(str(path)) as binary:
            for imported in getattr(binary, "DIRECTORY_ENTRY_IMPORT", []):
                dependency = imported.dll.decode().lower()
                if dependency in available and dependency not in copied:
                    pending.append(dependency)
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    subprocess.run([str(target / "dot.exe"), "-c"], check=True, creationflags=flags)
    test = subprocess.run([str(target / "dot.exe"), "-Tplain"], input=b"digraph { a -> b }", capture_output=True, creationflags=flags, check=True)
    if b"edge a b" not in test.stdout:
        raise RuntimeError("Portable Graphviz smoke test failed")
    return target.parent


def prepare_licenses():
    folder = ROOT / "licenses"
    folder.mkdir(exist_ok=True)
    sources = {
        "Graphviz-COPYING.txt": "https://gitlab.com/api/v4/projects/4207231/repository/files/epl-v10.txt/raw?ref=12.2.1",
        "Qt-LGPL-3.0.txt": "https://raw.githubusercontent.com/qt/qtbase/v6.10.1/LICENSES/LGPL-3.0-only.txt",
        "Qt-GPL-3.0.txt": "https://raw.githubusercontent.com/qt/qtbase/v6.10.1/LICENSES/GPL-3.0-only.txt",
        "Expat-MIT.txt": "https://raw.githubusercontent.com/libexpat/libexpat/R_2_6_4/expat/COPYING",
        "zlib-LICENSE.txt": "https://raw.githubusercontent.com/madler/zlib/v1.3.1/LICENSE",
    }
    for name, url in sources.items():
        path = folder / name
        if not path.exists():
            with urllib.request.urlopen(url, timeout=40) as response:
                data = response.read()
            if len(data) < 100 or data.startswith(b"<!DOCTYPE"):
                raise RuntimeError("Unexpected license response: " + url)
            path.write_bytes(data)
    shutil.copy2(ROOT.parent / "LICENSE", folder / "HNU-Script-MIT.txt")
    python_license = Path(sys.base_prefix) / "LICENSE.txt"
    if python_license.exists():
        shutil.copy2(python_license, folder / "Python-LICENSE.txt")
    (folder / "sources.json").write_text(json.dumps(sources, indent=2), encoding="utf-8")
    return folder


def main():
    if sys.platform != "win32" or sys.version_info[:2] != (3, 11):
        raise RuntimeError("Build with Python 3.11 on Windows x64")
    graphviz = graphviz_runtime()
    licenses = prepare_licenses()
    version_file = ROOT / "build/version_info.txt"
    numbers = tuple(map(int, VERSION.split("."))) + (0,)
    version_file.write_text(f'''VSVersionInfo(ffi=FixedFileInfo(filevers={numbers!r}, prodvers={numbers!r}, mask=0x3f, flags=0x0, OS=0x40004, fileType=0x1, subtype=0x0, date=(0,0)), kids=[StringFileInfo([StringTable('040904B0', [StringStruct('FileDescription', 'Compiler Lexer Killer'), StringStruct('FileVersion', '{VERSION}'), StringStruct('ProductName', 'Compiler Lexer Killer'), StringStruct('ProductVersion', '{VERSION}')])]), VarFileInfo([VarStruct('Translation', [1033,1200])])])''', encoding="utf-8")
    command = [sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean", "--windowed", "--onedir", "--noupx",
               "--name", NAME, "--distpath", str(ROOT / "dist"), "--workpath", str(ROOT / "build/pyinstaller"),
               "--specpath", str(ROOT / "build"), "--version-file", str(version_file),
               "--add-data", f"{ROOT / 'compiler_lexer_killer/presets'};compiler_lexer_killer/presets",
               "--add-data", f"{ROOT / 'compiler_lexer_killer/ui/styles.qss'};compiler_lexer_killer/ui",
               "--add-data", f"{graphviz};vendor/graphviz",
               "--add-data", f"{licenses};licenses", str(ROOT / "main.py")]
    build_env = os.environ.copy()
    # Prevent unrelated SDKs on the developer PATH from supplying incompatible
    # DLLs (e.g. Poppler's versioned ICU instead of the Windows ICU Qt requires).
    for key in tuple(build_env):
        if key.upper().startswith(("QT_", "QML", "PYTHONPATH", "PYTHONHOME")):
            del build_env[key]
    windows = Path(build_env.get("SYSTEMROOT", "C:/Windows"))
    build_env["PATH"] = os.pathsep.join(map(str, (Path(sys.executable).parent, Path(sys.base_prefix), windows / "System32", windows)))
    build_env["PYINSTALLER_CONFIG_DIR"] = str(ROOT / "build/pyinstaller-cache")
    subprocess.run(command, cwd=ROOT, env=build_env, check=True)
    destination = ROOT / "dist" / NAME
    # One coherent, backward-compatible VC runtime for Python and Qt. Preserve
    # Graphviz's own copy in its subprocess directory.
    qt = Path(PySide6.__file__).parent
    for runtime in qt.glob("*.dll"):
        if runtime.name.lower().startswith(("msvcp140", "vcruntime140")):
            shutil.copy2(runtime, destination / "_internal" / runtime.name)
    shutil.copy2(ROOT / "README.md", destination / "README.md")
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", destination / "THIRD_PARTY_NOTICES.md")
    shutil.copy2(ROOT / "VALIDATION.md", destination / "VALIDATION.md")
    if (ROOT / "examples").exists():
        shutil.copytree(ROOT / "examples", destination / "examples", dirs_exist_ok=True)
    shutil.copy2(ROOT / "vendor/graphviz-checksum.txt", destination / "graphviz-checksum.txt")
    # Ship the complete application source alongside dynamic Qt libraries.
    with zipfile.ZipFile(destination / "source.zip", "w", zipfile.ZIP_DEFLATED) as archive:
        for base in (ROOT / "compiler_lexer_killer", ROOT / "tests", ROOT / "tools", ROOT / "licenses", ROOT / "examples"):
            for path in sorted(base.rglob("*")):
                if path.is_file() and "__pycache__" not in path.parts:
                    archive.write(path, path.relative_to(ROOT))
        for name in ("main.py", "pyproject.toml", "requirements.txt", "requirements-dev.txt", "requirements-lock.txt", "README.md", "VALIDATION.md", "THIRD_PARTY_NOTICES.md"):
            archive.write(ROOT / name, name)
    archive = Path(shutil.make_archive(str(ROOT / "dist" / f"{NAME}-{VERSION}-win64"), "zip", ROOT / "dist", NAME))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix(".zip.sha256").write_text(f"{digest}  {archive.name}\n", encoding="ascii")
    print(f"Portable archive: {archive}\nSHA256: {digest}")


if __name__ == "__main__":
    main()
