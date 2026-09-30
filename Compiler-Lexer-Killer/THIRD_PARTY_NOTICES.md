# Third-party components

The application source follows the repository's MIT license. The portable folder
contains unmodified third-party runtimes with their own licenses. Qt DLLs remain
separate and replaceable. Application source is included as `source.zip`.

- Python 3.11: Python Software Foundation license; PyInstaller includes Python's
  license in `_internal`. Source: https://www.python.org/downloads/source/
- PySide6 / Shiboken6 / Qt 6.10.1: LGPLv3 / GPLv3 or commercial terms as provided by
  upstream. This build uses the open-source distribution. License texts are in
  `_internal/licenses`. Corresponding upstream sources:
  https://download.qt.io/official_releases/QtForPython/pyside6/PySide6-6.10.1-src/
  and https://download.qt.io/archive/qt/6.10/6.10.1/submodules/ .
- Graphviz 12.2.1: Eclipse Public License 1.0, the file referenced by upstream
  COPYING is included. Source:
  https://gitlab.com/graphviz/graphviz/-/tree/12.2.1 . Only `dot`, the core and dot
  layout plugins, and their DLL dependencies are included, without modification.
- Graphviz's zlib and Expat dependencies: zlib license and MIT license respectively;
  texts are included. Upstream source: https://github.com/madler/zlib and
  https://github.com/libexpat/libexpat .
- Microsoft Visual C++ runtime DLLs are redistributable runtime components supplied
  by the upstream Windows binary distributions and remain under Microsoft's terms.
- PyInstaller 6.16.0: GPL with its bootloader exception permitting distribution of
  bundled applications. Source and licensing: https://pyinstaller.org/ .

The application does not prevent replacing or debugging the dynamically linked
open-source libraries. Upstream copyright and license notices are preserved in
the included license files. See `licenses/sources.json` for the exact downloaded
notice sources used for the build.
