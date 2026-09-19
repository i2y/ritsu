import json, base64, os, time, urllib.request, urllib.parse, glob
LAWS = {
 '342AC0000000023': ['AppdxTable[1]'],
 '332AC0000000026': ['MainProvision-Article_91'],
 '340AC0000000033': ['MainProvision-Article_89','MainProvision-Article_28','MainProvision-Article_86','MainProvision-Article_84','MainProvision-Article_84_2','MainProvision-Article_83','MainProvision-Article_83_2','MainProvision-Article_185','MainProvision-Article_186','AppdxTable[2]','AppdxTable[4]'],
 '211AC0000000070': ['MainProvision-Article_40','MainProvision-Article_45','MainProvision-Article_160','MainProvision-Article_161','MainProvision-Article_3'],
 '329AC0000000115': ['MainProvision-Article_20','MainProvision-Article_24_4','MainProvision-Article_81','MainProvision-Article_82','MainProvision-Article_12'],
}
def path(id,date,elm): return f"cache/{id}/{date}/{elm.replace('[','_').replace(']','')}.xml"
todo=[]
for id,elms in LAWS.items():
    j=json.load(open(f'revisions/rev_{id}.json'))
    dates=sorted({r['amendment_enforcement_date'] for r in j['revisions'] if r.get('amendment_enforcement_date')})
    for elm in elms:
        for d in dates:
            p=path(id,d,elm)
            if not os.path.exists(p) or open(p,'rb').read()==b'ERR': todo.append((id,d,elm,p))
print('todo',len(todo),flush=True)
backoff=5
for id,d,elm,p in todo:
    os.makedirs(os.path.dirname(p),exist_ok=True)
    url=f'https://laws.e-gov.go.jp/api/2/law_data/{id}?asof={d}&elm={urllib.parse.quote(elm)}&law_full_text_format=xml'
    while True:
        try:
            with urllib.request.urlopen(url,timeout=120) as r: j=json.load(r)
            x=base64.b64decode(j['law_full_text']); open(p,'wb').write(x); open(p+'.rev','w').write(j.get('revision_info',{}).get('law_revision_id','')); print('ok',p,len(x),flush=True); backoff=5; break
        except urllib.error.HTTPError as e:
            if e.code in (404,400): open(p,'wb').write(b''); open(p+'.rev','w').write('404'); print('404',p,flush=True); break
            print('http',e.code,p,'sleep',backoff,flush=True); time.sleep(backoff); backoff=min(backoff*2,120)
        except Exception as e:
            print('err',e,p,'sleep',backoff,flush=True); time.sleep(backoff); backoff=min(backoff*2,120)
    time.sleep(0.8)
print('FINISH DONE',flush=True)
