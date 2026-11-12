#!/usr/bin/env python3
"""Validate the source bundle as well as the standalone Rust workspace."""
import json
import pathlib
import subprocess
import sys
import tomllib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from package import MAX_FILES, allowed  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read_toml(path):
    return tomllib.loads((ROOT / path).read_text())


def main():
    compatibility = json.loads((ROOT / 'compatibility.json').read_text())
    native = json.loads((ROOT / 'horizon-native.json').read_text())
    mixin = read_toml('mixins/aei/horizon-mixin.toml')
    package = read_toml('Cargo.toml')['package']
    engine = read_toml('mixins/aei/engine/Cargo.toml')['package']
    for key, expected in [('game', compatibility['game']), ('hooks', compatibility['hooks']),
                          ('horizon', compatibility['horizon_revision']),
                          ('mixins', compatibility['horizon_mixins_revision']),
                          ('id', mixin['mod']['id']), ('version', package['version'])]:
        assert native[key] == expected, f'horizon-native.json: {key} differs from source bundle'
    assert mixin['compatibility']['horizon_revision'] == native['horizon']
    assert mixin['mod']['version'] == engine['version'] == package['version']
    for module in mixin['module']:
        assert (ROOT / 'mixins/aei' / module['source']).is_file(), module['source']
    for injection in mixin['injection']:
        assert (ROOT / 'mixins/aei' / injection['helper']).is_file(), injection['helper']
    bundle = ROOT / 'mixins/aei'
    bundled = [p for p in bundle.rglob('*') if p.is_file() and 'target' not in p.relative_to(bundle).parts]
    for path in bundled:
        assert not path.is_symlink(), f'{path}: links are refused by Horizon'
        assert allowed(path.relative_to(bundle).as_posix()), f'{path}: Horizon drops this file from the bundle'
    assert len(bundled) < MAX_FILES, 'bundle exceeds the registry file limit'
    for folder in ['src', 'mixins', 'tools', 'docs']:
        for path in (ROOT / folder).rglob('*'):
            assert path.suffix.lower() not in {'.hnmod', '.jar', '.zip', '.wasm'}, path
    assert not list(ROOT.glob('*.hnmod')), 'Keep packaged mods outside the source tree'
    assert not (ROOT / 'mixins/aei/engine/data/jei').exists(), 'JEI assets are prohibited'
    subprocess.run(['cargo', 'fmt', '--all', '--check'], cwd=ROOT, check=True)
    helpers = sorted(str(p.relative_to(ROOT)) for p in (ROOT / 'mixins/aei').glob('*.rs'))
    subprocess.run(['rustfmt', '--edition', '2024', '--check', *helpers], cwd=ROOT, check=True)
    subprocess.run(['cargo', 'clippy', '--workspace', '--all-targets', '--offline', '--', '-D', 'warnings'], cwd=ROOT, check=True)
    subprocess.run(['cargo', 'test', '--workspace', '--offline'], cwd=ROOT, check=True)
    print('Bundle metadata, source hygiene, formatting, Clippy and tests passed.')


if __name__ == '__main__':
    main()
