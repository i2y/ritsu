import re,sys
sys.path.insert(0,'.')
from xt import xml_text
id,numpat=sys.argv[1],sys.argv[2]
pat=sys.argv[3] if len(sys.argv)>3 else None
x=__import__('gzip').open(f'laws/full_{id}.xml.gz','rt',encoding='utf-8').read()
for attrs,body in re.findall(r'<SupplProvision\b([^>]*)>(.*?)</SupplProvision>',x,flags=re.S):
    m=re.search(r'AmendLawNum="([^"]*)"',attrs)
    if m and re.search(numpat,m.group(1)):
        lines=xml_text(body)
        print(f'--- {m.group(1)}: {len(lines)} lines')
        for l in lines:
            if pat is None or re.search(pat,l): print('  '+l[:500])
