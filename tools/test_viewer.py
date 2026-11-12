#!/usr/bin/env python3
"""Exercise an AEI test build already running in a disposable survival world.

Usage: python3 tools/test_viewer.py /tmp/test-game/ctl.sock --out /tmp/aei-ui
No game files or screenshots are written into the source repository.
"""
import argparse
import json
import pathlib
import socket


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('socket', type=pathlib.Path)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    connection = socket.socket(socket.AF_UNIX)
    connection.settimeout(45)
    connection.connect(str(args.socket))
    stream = connection.makefile('rwb', buffering=0)

    def command(line):
        stream.write((line + '\n').encode())
        reply = json.loads(stream.readline())
        assert 'error' not in reply, (line, reply)
        return reply

    def ui():
        return command('ui')['widgets']

    def widget(label):
        return next(w for w in ui() if w.get('label') == label)

    def click(label, right=False):
        command('click ' + widget(label)['id'] + (' right' if right else ''))

    def ingredients():
        return [w['label'] for w in ui() if w.get('label', '').startswith('Recipe ') and ' ingredient ' in w['label']]

    command('options-set tutorialStep none')
    if command('state')['screen'] != 'inventory':
        command('open-inventory')
    for _ in range(20):
        if not ingredients():
            break
        command('key esc tap')
    initial_field = next(w for w in ui() if w['kind'] == 'field')
    command('click ' + initial_field['id'])
    command('key ctrl down')
    command('key a tap')
    command('key ctrl up')
    for _ in range(50):
        command(r'type \b')
    click('Stone', right=True)
    first = ingredients()
    assert first and any(label.startswith('Recipe 1 ') for label in first), first
    click('Next recipe page')
    second = ingredients()
    assert first != second, (first, second)
    command('scroll-at 210 120 -1')
    assert ingredients() != second
    click('Recipe category 1')
    assert ingredients()
    click('Recipe category 0')
    click('Show all recipes in category')
    assert ingredients()

    # Modal blank space must consume clicks; history works without a hovered item.
    command('click-at 1 1')
    command('key left tap')
    assert ingredients()
    command('key left tap')
    assert not ingredients()
    command('key right tap')
    assert ingredients()

    field = next(w for w in ui() if w['kind'] == 'field')
    command('click ' + field['id'])
    command('type nonexistent-aei-fixture')
    assert not any(w.get('label', '').startswith('Item ') for w in ui())
    command('key ctrl down')
    command('key a tap')
    command('key ctrl up')
    command('click ' + field['id'] + ' right')
    command('type oak')
    fields = [w for w in ui() if w['kind'] == 'field']
    assert any(w.get('label', '').startswith('Item ') for w in ui()), fields
    command('click ' + fields[0]['id'] + ' right')
    command('click-at 1 1')
    state = command('state')
    assert state['screen'] == 'inventory'
    command('screenshot ' + str((args.out / 'recipes.png').resolve()))
    command('key esc tap')
    assert not ingredients()
    command('screenshot ' + str((args.out / 'catalogue.png').resolve()))
    (args.out / 'result.json').write_text(json.dumps({'status': 'passed', 'checks': ['recipe pages', 'scroll', 'categories', 'category recipes', 'modal clicks', 'history', 'clear search', 'typing', 'return to catalogue']}, indent=2) + '\n')
    print('Viewer interaction checks passed; screenshots:', args.out)


if __name__ == '__main__':
    main()
