"""Rebuild cache/<law>/<asof>/<elm>.xml from fragments/index.tsv, so that report.py runs."""
import os, shutil
for line in open('fragments/index.tsv', encoding='utf-8').read().splitlines()[1:]:
    law, elm, asof, rev, h = line.split('\t')
    d = f'cache/{law}/{asof}'
    os.makedirs(d, exist_ok=True)
    p = f'{d}/{elm}.xml'
    if h == '-':
        open(p, 'wb').close()
    else:
        shutil.copyfile(f'fragments/{law}_{elm}_{h}.xml', p)
    open(p + '.rev', 'w').write(rev)
print('cache rebuilt')
