#!/usr/bin/env python3
"""Decode a rzr-captura folder: list the headset commands Synapse sent during
each capture step, with the decoded result Synapse logged for each.

Usage: python3 decodificar-log.py CAPTURE_DIR [--all]
  CAPTURE_DIR  unzipped rzr-captura (contains pasos.txt and logs/)
  --all        also show THX / Windows-audio log lines, not only USB commands
"""
import json
import os
import re
import sys


def find_product_log(root):
    for dirpath, _, files in os.walk(root):
        for f in files:
            name = os.path.join(dirpath, f)
            # zips made on Windows unpack with literal backslashes on Linux
            if re.search(r'products_1365_mw.*\.log$', name.replace('\\', '/')):
                return name
    sys.exit('products_1365_mw log not found')


def secs(t):
    h, m, s = t.split(':')
    return int(h) * 3600 + int(m) * 60 + float(s)


def main():
    root = sys.argv[1]
    show_all = '--all' in sys.argv
    steps = []
    for line in open(os.path.join(root, 'pasos.txt'), encoding='utf-8-sig'):
        m = re.match(r'(\d\d:\d\d:\d\d\.\d+)\s+(\S+)\s+(.*)', line.strip())
        if m:
            steps.append(m.groups())

    events = []
    for line in open(find_product_log(root), encoding='utf-8', errors='replace'):
        m = re.match(r'\[\d{4}/\d\d/\d\d (\d\d:\d\d:\d\d\.\d+)\] \w+: (.*)', line)
        if not m:
            continue
        t, msg = m.groups()
        if 'sendCommandOut() dataSend' in msg:
            d = json.loads(msg.split('dataSend:', 1)[1])
            b = [d[str(i)] for i in range(64)]
            if b[10] == 0xE1:
                continue  # remote-mode frames
            data = ' '.join(f'{x:02X}' for x in b[13:13 + b[12]])
            events.append((secs(t), t, f'OUT {b[9]:02X}/{b[10]:02X} flag={b[11]:02X} [{data}]'))
        elif re.match(r'protocolAudio\.\w+ r->', msg) and 'Remote Mode' not in msg:
            fn = re.match(r'protocolAudio\.(\w+)', msg).group(1)
            js = re.search(r'"jsonData":(\{[^}]*\})', msg)
            events.append((secs(t), t, f'    {fn} -> {js.group(1) if js else ""}'))
        elif 'registeredEvent' in msg:
            ev = re.findall(r'"recordIdDesc":"([^"]+)","data":(\{[^}]*\})', msg)
            ev = list(dict.fromkeys(ev))
            events.append((secs(t), t, f'  EVENT {ev}'))
        elif show_all and re.search(r'AudioEffectsTHXV3|RzSimpleService\.simpleSet', msg):
            events.append((secs(t), t, f'  ~ {msg[:200]}'))
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
