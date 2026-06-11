#!/usr/bin/env python3
"""Generate SHA256 checksum files for model assets.

The script walks up the directory hierarchy from its location until it finds
the `assets/search/models` directory. When the directory is found, it
computes SHA‑256 hashes for `model_quantized.onnx` and
`model_quantized.onnx_data` (if present) and writes the hashes to adjacent
`.sha256` files. These checksum files can be committed to Git and are used
by the build script for integrity verification.
"""
import hashlib
import pathlib
import sys

def sha256_of_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()

def find_repo_root(start: pathlib.Path) -> pathlib.Path:
    """Walk upwards until a directory containing `assets/search/models` is found.
    Returns the directory path or exits with error if not found.
    """
    for ancestor in start.resolve().parents:
        candidate = ancestor / "assets" / "search" / "models"
        if candidate.is_dir():
            return ancestor
    sys.stderr.write("Unable to locate assets/search/models in any parent directory.\n")
    sys.exit(1)

def main():
    script_dir = pathlib.Path(__file__).parent
    repo_root = find_repo_root(script_dir)
    models_dir = repo_root / "assets" / "search" / "models"
    for name in ["model_quantized.onnx", "model_quantized.onnx_data"]:
        model_path = models_dir / name
        if not model_path.is_file():
            sys.stderr.write(f"Model file missing (skip): {model_path}\n")
            continue
        checksum = sha256_of_file(model_path)
        checksum_path = model_path.with_name(f"{name}.sha256")
        checksum_path.write_text(f"{checksum}  {name}\n")
        print(f"Wrote {checksum_path}")

if __name__ == "__main__":
    main()
