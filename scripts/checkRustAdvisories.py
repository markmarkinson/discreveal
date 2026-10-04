"""Read-only OSV check of the actual Windows dependency graph; fail on service errors."""
import json
import pathlib
import re
import subprocess
import sys
import tomllib
import urllib.request

root = pathlib.Path(__file__).resolve().parent.parent
lock = tomllib.loads((root / 'src-tauri/Cargo.lock').read_text(encoding='utf-8'))
tree = subprocess.check_output(['cargo', 'tree', '--manifest-path', 'src-tauri/Cargo.toml',
    '--locked', '--edges', 'normal,build', '--target', 'x86_64-pc-windows-msvc', '--prefix', 'none', '--format', '{p}'], cwd=root, text=True)
active = set(re.findall(r'^([\w-]+) v([\w.+-]+)', tree, flags=re.MULTILINE))
packages = [p for p in lock['package'] if p.get('source', '').startswith('registry+') and (p['name'], p['version']) in active]
hits = []
for offset in range(0, len(packages), 100):
    batch = packages[offset:offset+100]
    payload = {'queries': [{'package': {'name': p['name'], 'ecosystem': 'crates.io'}, 'version': p['version']} for p in batch]}
    request = urllib.request.Request('https://api.osv.dev/v1/querybatch', data=json.dumps(payload).encode(), headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(request, timeout=30) as response:
        results = json.load(response)['results']
    if len(results) != len(batch):
        raise RuntimeError('Incomplete advisory response')
    for package, result in zip(batch, results):
        for advisory in result.get('vulns', []):
            if not advisory.get('withdrawn'):
                hits.append({'package':package['name'], 'version':package['version'], 'advisory':advisory['id']})
print(json.dumps({'target':'x86_64-pc-windows-msvc','packagesChecked':len(packages),'hits':hits}, indent=2))
sys.exit(1 if hits else 0)
