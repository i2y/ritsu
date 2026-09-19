import re,sys,json
sys.path.insert(0,'.')
from xt import xml_text
names={'342AC0000000023':'印紙税法','332AC0000000026':'租税特別措置法','340AC0000000033':'所得税法','211AC0000000070':'健康保険法','329AC0000000115':'厚生年金保険法'}
res={}
for id,nm in names.items():
    x=__import__('gzip').open(f'laws/full_{id}.xml.gz','rt',encoding='utf-8').read()
    sps=re.findall(r'<SupplProvision\b([^>]*)>(.*?)</SupplProvision>',x,flags=re.S)
    tot=len(sps); juzen=0; tekiyo=0; both=0; neither=0; empty=0
    rows=[]
    for attrs,body in sps:
        m=re.search(r'AmendLawNum="([^"]*)"',attrs)
        num=m.group(1) if m else '(原始)'
        t='\n'.join(xml_text(body))
        a='従前の例' in t
        b=bool(re.search(r'以後に.{0,40}について適用し',t)) or bool(re.search(r'分以後の.{0,20}について適用',t)) or bool(re.search(r'以後の.{0,30}について適用',t))
        if a and b: both+=1
        elif a: juzen+=1
        elif b: tekiyo+=1
        else: neither+=1
        rows.append((num,len(t),a,b))
    res[nm]=dict(total=tot,従前のみ=juzen,適用のみ=tekiyo,両方=both,どちらも無し=neither)
    print(nm,res[nm])
json.dump(res,open('suppl_counts.json','w'),ensure_ascii=False,indent=1)
