"""Fetch the official portable Graphviz archive and verify its publisher checksum."""
import hashlib
import shutil
import urllib.request
import zipfile
from pathlib import Path

VERSION = "12.2.1"
URL = f"https://gitlab.com/api/v4/projects/4207231/packages/generic/graphviz-releases/{VERSION}/windows_10_cmake_Release_Graphviz-{VERSION}-win64.zip"
ROOT = Path(__file__).resolve().parents[1]


def main():
    vendor = ROOT / "vendor"
    vendor.mkdir(exist_ok=True)
    archive = vendor / f"graphviz-{VERSION}.zip"
    with urllib.request.urlopen(URL + ".sha256", timeout=60) as response:
        expected = response.read().decode().split()[0].lower()
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest() != expected:
        with urllib.request.urlopen(URL, timeout=60) as response, archive.open("wb") as output:
            shutil.copyfileobj(response, output)
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    if actual != expected:
        raise RuntimeError("Graphviz SHA256 verification failed")
    target = vendor / "graphviz"
    target.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive) as zipped:
        for member in zipped.infolist():
            parts = Path(member.filename).parts
            # Archive contains a versioned top-level directory.
            relative = Path(*parts[1:])
            dest = (target / relative).resolve()
            if not dest.is_relative_to(target.resolve()):
                raise RuntimeError("Unsafe archive path")
            if member.is_dir():
                dest.mkdir(parents=True, exist_ok=True)
            else:
                dest.parent.mkdir(parents=True, exist_ok=True)
                with zipped.open(member) as source, dest.open("wb") as output:
                    shutil.copyfileobj(source, output)
    (vendor / "graphviz-checksum.txt").write_text(f"Graphviz {VERSION}\n{URL}\nSHA256 {actual}\n", encoding="utf-8")
    print(f"Verified Graphviz {VERSION}: {actual}")


if __name__ == "__main__":
    main()
