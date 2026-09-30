tpl=open('template.html').read()
for name,scale,pos in [('portrait','1','42%'),('wide','0.85','72%'),('sky','0.28','50%')]:
    open(name+'.html','w').write(tpl.replace('__SCALE__',scale).replace('__PHOTOPOS__',pos))
