#!/usr/bin/env python3
"""Build the BeamNG mod ZIP; optionally copy it into an explicitly chosen mods folder."""
import argparse
from pathlib import Path
import shutil
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mods-dir", type=Path, help="Existing BeamNG user-folder/mods directory")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    output = root / "dist" / "daqapp_telemetry.zip"
    output.parent.mkdir(exist_ok=True)
    files = sorted((root / "mod").rglob("*.lua"))
    if not files:
        parser.error("No mod Lua files found")
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
        for file in files:
            archive.write(file, file.relative_to(root / "mod").as_posix())
    print(output)
    if args.mods_dir:
        mods = args.mods_dir.expanduser().resolve()
        if not mods.is_dir() or mods.name != "mods":
            parser.error("--mods-dir must be an existing directory named 'mods'")
        destination = mods / output.name
        shutil.copy2(output, destination)
        print(f"Installed: {destination}")


if __name__ == "__main__":
    main()
