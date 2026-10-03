import re,sys,difflib
BREAKS={"Paragraph","Item","Subitem1","Subitem2","ArticleCaption","ArticleTitle","AppdxTableTitle","RelatedArticleNum","TableRow","Sentence","TableStructTitle"}
def xml_text(x):
    out=[];rest=x
    while True:
        lt=rest.find('<')
        if lt<0: out.append(rest); break
        out.append(rest[:lt]); gt=rest.find('>',lt)
        if gt<0: break
        tag=rest[lt+1:gt]
        if tag.startswith('/'):
            n=tag[1:].strip()
            if n in BREAKS: out.append('\n')
            elif n in ('TableColumn','Column'): out.append(' ')
        rest=rest[gt+1:]
    s=''.join(out); lines=[]
    for l in s.split('\n'):
        t=' '.join(l.split())
        if t and (not lines or lines[-1]!=t): lines.append(t)
    return lines
def diff(a,b):
    la,lb=xml_text(a),xml_text(b)
    return [l for l in difflib.unified_diff(la,lb,lineterm='',n=1) if not l.startswith(('---','+++'))]
if __name__=='__main__':
    a=open(sys.argv[1],encoding='utf-8').read(); b=open(sys.argv[2],encoding='utf-8').read()
    for l in diff(a,b): print(l)
