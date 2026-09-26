#!/usr/bin/env python3
"""Decode an rzr-capture folder: for each capture step, list the HID frames
Synapse sent to the headset and what it logged about them.

Usage: python3 decode-synapse-log.py CAPTURE_DIR [--all]
  CAPTURE_DIR  the unzipped rzr-capture folder (steps.txt and logs/)
  --all        also show THX / Windows audio lines, not only HID frames

Synapse writes one products_<product id>_mw*.log per device (the product id
in decimal: 1365 = 0x0555). Frames of the Audio MXIC ("PA") protocol are
shown decoded (type/command, flag, data); any other protocol as raw bytes.
"""
import json
import os
import re
import sys


def product_logs(root):
    found = []
    for dirpath, _, files in os.walk(root):
        for f in files:
            name = os.path.join(dirpath, f)
            # zips made on Windows unpack with literal backslashes on Linux
            m = re.search(r'products_(\d+)_mw.*\.log$', name.replace('\\', '/'))
            if m:
                found.append((int(m.group(1)), name))
    if not found:
        sys.exit('no products_*_mw*.log found in ' + root)
    return found


def secs(t):
    h, m, s = t.split(':')
    return int(h) * 3600 + int(m) * 60 + float(s)


def frame_text(b):
    if len(b) > 12 and b[5:7] == [0x50, 0x41]:  # "PA": Audio MXIC
        if b[10] == 0xE1:
            return None  # remote-mode frames: in every sequence, just noise
        data = ' '.join(f'{x:02X}' for x in b[13:13 + b[12]])
        return f'OUT {b[9]:02X}/{b[10]:02X} flag={b[11]:02X} [{data}]'
    end = max((i + 1 for i, x in enumerate(b) if x), default=0)
    return 'OUT ' + ' '.join(f'{x:02X}' for x in b[:end])


def events_of(path, pid, show_all):
    events = []
    for line in open(path, encoding='utf-8', errors='replace'):
        m = re.match(r'\[\d{4}/\d\d/\d\d (\d\d:\d\d:\d\d\.\d+)\] \w+: (.*)', line)
        if not m:
            continue
        t, msg = m.groups()
        tag = f'{pid:04X}'
        if 'dataSend' in msg:
            try:
                d = json.loads(msg.split('dataSend:', 1)[1])
            except ValueError:
                continue
            b = [d[k] for k in sorted(d, key=int)] if isinstance(d, dict) else list(d)
            text = frame_text(b)
            if text:
                events.append((secs(t), t, f'{tag} {text}'))
        elif re.match(r'protocol\w*\.\w+ r->', msg) and 'Remote Mode' not in msg:
            fn = re.match(r'protocol\w*\.(\w+)', msg).group(1)
            js = re.search(r'"jsonData":(\{[^}]*\})', msg)
            events.append((secs(t), t, f'{tag}     {fn} -> {js.group(1) if js else ""}'))
        elif 'registeredEvent' in msg:
            ev = re.findall(r'"recordIdDesc":"([^"]+)","data":(\{[^}]*\})', msg)
            events.append((secs(t), t, f'{tag}   EVENT {list(dict.fromkeys(ev))}'))
        elif show_all and re.search(r'AudioEffectsTHX|RzSimpleService\.simpleSet', msg):
            events.append((secs(t), t, f'{tag}   ~ {msg[:200]}'))
    return events


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    root = sys.argv[1]
    show_all = '--all' in sys.argv
    steps = []
    for line in open(os.path.join(root, 'steps.txt'), encoding='utf-8-sig'):
        m = re.match(r'(\d\d:\d\d:\d\d\.\d+)\s+(\S+)\s+(.*)', line.strip())
        if m:
            steps.append(m.groups())

    events = []
    for pid, path in product_logs(root):
        print(f'# {os.path.basename(path)}: product 0x{pid:04X}')
        events += events_of(path, pid, show_all)
    events.sort()

    prev = 0.0
    for t, sid, desc in steps:
        cur = secs(t)
        print(f'\n=== {sid}: {desc}')
        for et, ets, e in events:
            if prev < et <= cur:
                print(f'  {ets} {e}')
        prev = cur


if __name__ == '__main__':
    main()
