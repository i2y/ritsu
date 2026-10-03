import os,re,sys,json
sys.path.insert(0,'.')
from xt import diff, xml_text
names={'342AC0000000023':'印紙税法','332AC0000000026':'租税特別措置法','340AC0000000033':'所得税法','211AC0000000070':'健康保険法','329AC0000000115':'厚生年金保険法'}
NUM=re.compile(r'[〇一二三四五六七八九十百千万億0-9０-９]+')
def metrics(x):
    return dict(rows=x.count('<TableRow'), cols=x.count('<TableColumn'), paras=x.count('<Paragraph '), items=x.count('<Item '), sub=x.count('<Subitem1 '))
want=sys.argv[1:]  # law ids to report
for id in want:
    base=f'cache/{id}'
    if not os.path.isdir(base): continue
    dates=sorted(os.listdir(base))
    elms=sorted({f[:-4] for d in dates for f in os.listdir(f'{base}/{d}') if f.endswith('.xml')})
    for elm in elms:
        prev=None; changes=[]
        for d in dates:
            p=f'{base}/{d}/{elm}.xml'
            if not os.path.exists(p): continue
            x=open(p,encoding='utf-8').read()
            if x in ('','ERR'):  # 404 or failed
                if prev is None: prev=(d,x)
                continue
            if prev is not None and x!=prev[1]:
                if prev[1] in ('','ERR'):
                    changes.append((prev[0],d,'(new/404→text)',[],metrics(x),metrics(x)))
                else:
                    dl=diff(prev[1],x)
                    changes.append((prev[0],d,None,dl,metrics(prev[1]),metrics(x)))
            prev=(d,x)
        print(f'##### {names[id]} {elm}: {len(changes)} changes over {len(dates)} dates')
        for a,b,note,dl,m0,m1 in changes:
            rem=' '.join(l[1:] for l in dl if l.startswith('-')); add=' '.join(l[1:] for l in dl if l.startswith('+'))
            n0=sorted(NUM.findall(rem)); n1=sorted(NUM.findall(add))
            kind = note or ('形式のみ' if not dl else ('数値変化' if n0!=n1 else '語句のみ'))
            struct = '' if m0==m1 else f' 構造 {m0}->{m1}'
            print(f'--- {a} -> {b}: {kind}, difflines={len(dl)}{struct}')
            for l in dl[:10]: print('    '+l[:220])
