var css = [];
for (var i=0; i<document.styleSheets.length; i++)
{
  try {
    var sheet = document.styleSheets[i];
    var rules = ('cssRules' in sheet)? sheet.cssRules : sheet.rules;
    if (rules)
    {
        css.push('/* Stylesheet : '+(sheet.href||'[inline styles]')+' */');
        for (var j=0; j<rules.length; j++)
        {
            var rule = rules[j];
            if ('cssText' in rule)
                css.push(rule.cssText);
            else
                css.push(rule.selectorText+' {'+rule.style.cssText+'}');
        }
    }
  } catch (e) {
    
  }
}
var cssInline = css.join('')+'';
setTimeout(() => navigator.clipboard.writeText(cssInline), 2000);