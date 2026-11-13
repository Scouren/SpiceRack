"""defaults.py: run params.py's table and keep the rows whose numeric defaults differ, or that exist on one side."""
import re, subprocess, sys
out = subprocess.run(['python3', sys.argv[1]] + sys.argv[2:], capture_output=True, text=True).stdout
def num(x):
    try: return float(x)
    except: return None
for l in out.split('\n'):
    m = re.match(r'(\S+)\s+C (\S*)\s+default (.*?)\s*\| VA (\S+)\s*(.*)$', l)
    if not m: continue
    name, field, cd, vn, vd = m.groups()
    if 'VA only' in vd: print('VA only:', name, vd.replace('(VA only)', '').strip()); continue
    if vn == 'ABSENT':
        if field != '?': print('C only:', name, field, cd)
        continue
    a, b = num(cd), num(vd)
    if a is not None and b is not None and a != b: print('default differs:', name, 'C', cd, 'VA', vd)
