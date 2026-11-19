"""Expand a Hunspell dictionary (index.dic and index.aff) into lowercase word forms.

Usage: python3 unmunch.py PACKAGE_DIR OUT.txt

Keeps only forms that are ASCII letters and at least three long.
"""
import re,sys
pkg=sys.argv[1]
pfx={};sfx={}
cur=None
for line in open(f'{pkg}/index.aff',encoding='utf8'):
    p=line.split()
    if len(p)>=4 and p[0] in('PFX','SFX') and p[2] in('Y','N') and len(p)<=5 and p[3].isdigit():
        cur=(p[0],p[1],p[2]=='Y'); (pfx if p[0]=='PFX' else sfx).setdefault(p[1],{'cross':p[2]=='Y','rules':[]}); continue
    if len(p)>=5 and p[0] in('PFX','SFX'):
        strip=''if p[2]=='0' else p[2]; add=''if p[3]=='0' else p[3].split('/')[0]; cond=p[4]
        tbl=pfx if p[0]=='PFX' else sfx
        rx=re.compile(('^' if p[0]=='PFX' else '')+cond+('' if p[0]=='PFX' else '$')) if cond!='.' else None
        tbl[p[1]]['rules'].append((strip,add,rx))
def apply(word,tbl,flag,is_pfx):
    out=[]
    for strip,add,rx in tbl[flag]['rules']:
        if rx and not rx.search(word): continue
        if is_pfx:
            if strip and not word.startswith(strip): continue
            out.append(add+word[len(strip):])
        else:
            if strip and not word.endswith(strip): continue
            out.append(word[:len(word)-len(strip)]+add)
    return out
words=set()
lines=open(f'{pkg}/index.dic',encoding='utf8').read().split('\n')[1:]
for l in lines:
    if not l.strip(): continue
    w,_,flags=l.partition('/')
    flags=flags.split('\t')[0].strip()
    forms={w}
    sf=[f for f in flags if f in sfx]; pf=[f for f in flags if f in pfx]
    sfxforms=set()
    for f in sf:
        for x in apply(w,sfx,f,False): sfxforms.add((x,f))
    for x,_f in sfxforms: forms.add(x)
    for f in pf:
        for x in apply(w,pfx,f,True):
            forms.add(x)
            if pfx[f]['cross']:
                for s,sfl in sfxforms:
                    if sfx[sfl]['cross']: 
                        for y in apply(s,pfx,f,True): forms.add(y)
    words|=forms
out=set()
for w in words:
    w=w.lower()
    if len(w)>=3 and w.isascii() and w.isalpha(): out.add(w)
out=sorted(out)
print(f"{len(out)} forms",file=sys.stderr)
open(sys.argv[2],'w').write('\n'.join(out)+'\n')
