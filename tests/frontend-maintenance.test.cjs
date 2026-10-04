const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const root=path.join(__dirname,'..');
function translations(){
 const source=fs.readFileSync(path.join(root,'renderer/i18n.js'),'utf8');
 const context={window:{},document:{documentElement:{},querySelectorAll:()=>[]}};
 vm.runInNewContext(source.replace('window.DiscRevealI18n = Object.freeze','globalThis.auditDictionaries=dictionaries;window.DiscRevealI18n = Object.freeze'),context);
 return {source,dictionaries:context.auditDictionaries,api:context.window.DiscRevealI18n};
}
function files(dir){return fs.readdirSync(dir,{withFileTypes:true}).flatMap(item=>item.isDirectory()?files(path.join(dir,item.name)):[path.join(dir,item.name)]);}
test('German and English cover the same interface keys and placeholder contracts',()=>{
 const {source,dictionaries}=translations();assert.ok(!source.includes('Object.assign(dictionaries.'));
 assert.deepEqual(Object.keys(dictionaries.de).sort(),Object.keys(dictionaries.en).sort());
 for(const key of Object.keys(dictionaries.en)) {
  const placeholders=value=>[...value.matchAll(/\{(\w+)\}/g)].map(match=>match[1]).sort();
  assert.deepEqual(placeholders(dictionaries.de[key]),placeholders(dictionaries.en[key]),key);
 }
});
test('all explicit backend error codes have usable translations in both languages',()=>{
 const {api}=translations();
 const source=files(path.join(root,'src-tauri/src')).filter(file=>file.endsWith('.rs')).map(file=>fs.readFileSync(file,'utf8')).join('\n');
 const codes=new Set([...source.matchAll(/AppError::(?:new|with_detail)\s*\(\s*"([^"]+)"/g)].map(match=>match[1]));
 for(const language of ['de','en']) {
  api.setLanguage(language);
  for(const code of codes)assert.notEqual(api.describeError({code,detail:'example'}),'error.'+code,code+' in '+language);
 }
});
test('numbers, fractions, dates and allocated-size explanations follow the chosen app language',()=>{
 const {api}=translations();
 api.setLanguage('de');assert.equal(api.formatNumber(1234567),'1.234.567');assert.equal(api.formatDecimal(12.3),'12,3');
 assert.equal(api.t('storageLogicalSize',{size:'12 MB'}),'Dateigröße: 12 MB');
 assert.equal(api.t('storagePartial',{size:'12 MB'}),'Mindestens 12 MB');
 assert.match(api.formatDate(Date.UTC(2026,9,3,12)),/3\.10\.2026/);
 api.setLanguage('en');assert.equal(api.formatNumber(1234567),'1,234,567');assert.equal(api.formatDecimal(12.3),'12.3');
 assert.equal(api.t('storageLogicalSize',{size:'12 MB'}),'File size: 12 MB');
 assert.equal(api.t('storagePartial',{size:'12 MB'}),'At least 12 MB');
 assert.match(api.formatDate(Date.UTC(2026,9,3,12)),/10\/3\/2026/);
});
