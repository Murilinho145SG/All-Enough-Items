#!/usr/bin/env python3
"""Pack the AEI bundle into a reproducible `.hnmod` for a GitHub release.

Usage: python3 tools/package.py [--out dist]

Writes `<out>/aei-<version>.hnmod` and its `.sha256`. The archive holds
`horizon-native.json` and the files under `mixins/aei/` that Horizon's
registry accepts (Rust, TOML, data and crate text files), plus the repository
LICENSE. Entries are sorted with fixed timestamps, so the same sources always
produce the same bytes. Run `tools/check.py` first.
"""
import argparse
import hashlib
import json
import pathlib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
TEXT_PREFIXES = ('README', 'LICENSE', 'LICENCE', 'COPYING', 'NOTICE')
MAX_FILES = 512
MAX_BYTES = 64 << 20


def allowed(rest):
    """Mirror of `horizon_mods::index::native::allowed` for a path under `mixins/<id>/`."""
    parts = rest.split('/')
    in_data = len(parts) >= 2 and (parts[0] == 'data' or (len(parts) >= 3 and parts[1] == 'data'))
    text = len(parts) <= 2 and (parts[-1].startswith(TEXT_PREFIXES) or parts[-1] == '.gitignore')
    return rest.endswith(('.rs', '.toml')) or in_data or text


def bundle_files(mod_id):
    """Every file of `mixins/<id>/` the bundle carries, as {relative path: bytes}."""
    folder = ROOT / 'mixins' / mod_id
    files = {}
    for path in sorted(folder.rglob('*')):
        if path.is_symlink():
            raise SystemExit(f'{path}: links are not allowed in a native mod')
        rest = path.relative_to(folder).as_posix()
        if path.is_file() and 'target' not in path.relative_to(folder).parts and allowed(rest):
            files[rest] = path.read_bytes()
    files.setdefault('LICENSE', (ROOT / 'LICENSE').read_bytes())
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--out', type=pathlib.Path, default=ROOT / 'dist')
    args = parser.parse_args()

    manifest = json.loads((ROOT / 'horizon-native.json').read_text())
    mod_id = manifest['id']
    files = bundle_files(mod_id)
    if len(files) + 1 > MAX_FILES or sum(map(len, files.values())) > MAX_BYTES:
        raise SystemExit('bundle exceeds the registry limit of 512 files or 64 MiB')

    entries = [('horizon-native.json', json.dumps(manifest, indent=2).encode())]
    entries += [(f'mixins/{mod_id}/{rest}', data) for rest, data in sorted(files.items())]

    args.out.mkdir(parents=True, exist_ok=True)
    target = args.out / f"{mod_id}-{manifest['version']}.hnmod"
    with zipfile.ZipFile(target, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name, data in entries:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)

    digest = hashlib.sha256(target.read_bytes()).hexdigest()
    target.with_name(target.name + '.sha256').write_text(f'{digest}  {target.name}\n')
    print(f'{target} ({len(entries)} files)\nsha256 {digest}')


if __name__ == '__main__':
    main()
