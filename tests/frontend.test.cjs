const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const app = fs.readFileSync(path.join(__dirname, '../renderer/app.js'), 'utf8');
function fn(name) {
    const start = app.indexOf(`function ${name}(`);
    let depth = 0, opening = app.indexOf('{', start);
    for (let i = opening; i < app.length; i++) {
        if (app[i] === '{') depth++;
        if (app[i] === '}' && --depth === 0) return app.slice(app.slice(start-6,start)==='async '?start-6:start, i + 1);
    }
    throw new Error(`Missing function ${name}`);
}
function duplicateHelpers(document) {
    const window = { DiscRevealUtil: {formatBytes:String}, DiscRevealI18n: {t:key=>key, formatNumber:String, formatDate:String, formatDecimal:String} };
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../renderer/operationProgress.js'), 'utf8'), { window, document });
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../renderer/duplicates.js'), 'utf8'), { window, document });
    return window.DiscRevealDuplicates;
}

test('duplicate finder opens before a drive scan, restores the start view and preserves an active search', () => {
    const element = (hidden = false) => {
        const classes = new Set(hidden ? ['hidden'] : []);
        return {setAttribute(name, value) {this[name] = value;}, classList: {
            contains: name => classes.has(name), add: name => classes.add(name),
            toggle(name, force) {if (force) classes.add(name); else classes.delete(name);},
        }};
    };
    const el = {duplicatesView:element(true), breadcrumbRow:element(true), emptyState:element(),
        btnDuplicates:element(), fileList:element(true), gridView:element(true)};
    let resets = 0;
    const state = {fullTree:null, duplicatesCleaning:false};
    const context = {el, state, abandonDuplicatesScan() {resets++;},
        setSearchActive() {}, setChartsActive() {}, renderDuplicatesView() {}, refreshDuplicatesCacheStats() {}};
    vm.runInNewContext([fn('isDuplicatesActive'), fn('setDuplicatesActive'), fn('openDuplicateFinder')].join('\n'), context);
    context.openDuplicateFinder();
    assert.equal(el.duplicatesView.classList.contains('hidden'), false);
    assert.equal(el.emptyState.classList.contains('hidden'), true);
    assert.equal(el.btnDuplicates['aria-pressed'], 'true');
    state.duplicatesPhase = 'scanning';
    context.openDuplicateFinder();
    assert.equal(state.duplicatesPhase, 'scanning');
    assert.equal(resets, 1);
    context.setDuplicatesActive(false);
    assert.equal(state.duplicatesPhase, 'scanning');
    assert.equal(el.emptyState.classList.contains('hidden'), false);
    assert.equal(el.breadcrumbRow.classList.contains('hidden'), true);
    assert.equal(el.btnDuplicates['aria-pressed'], 'false');
    state.fullTree = {path:'C:'};
    context.openDuplicateFinder();
    assert.equal(state.duplicatesPhase, 'scanning');
    assert.equal(resets, 1);
    context.setDuplicatesActive(false);
    assert.equal(el.emptyState.classList.contains('hidden'), true);
    assert.equal(el.breadcrumbRow.classList.contains('hidden'), false);
    state.duplicatesCleaning = true;
    const previousResets = resets;
    context.openDuplicateFinder();
    assert.equal(resets, previousResets);
});

class TestElement {
    constructor() {
        this.children=[]; this.className=''; this.dataset={}; this.style={};
        this.classList={toggle:(name,force)=>{const classes=new Set(this.className.split(' ').filter(Boolean));const active=force===undefined?!classes.has(name):force;if(active)classes.add(name);else classes.delete(name);this.className=[...classes].join(' ');}};
    }
    append(...children) { children.forEach(child=>this.appendChild(child)); }
    appendChild(child) { child.parent=this; this.children.push(child); }
    setAttribute(name,value) {this[name]=value;}
    removeAttribute(name) {delete this[name];}
    replaceChildren(...children) {this.children=[];this.append(...children);}
    replaceWith(child) {child.parent=this.parent;this.parent.children[this.parent.children.indexOf(this)]=child;}
    insertBefore(child,before) {child.parent=this;const index=this.children.indexOf(before);this.children.splice(index<0?this.children.length:index,0,child);}
    remove() {this.parent.children=this.parent.children.filter(child=>child!==this);}
    querySelector(selector) {return this.querySelectorAll(selector)[0]||null;}
    querySelectorAll(selector) {const match=node=>selector[0]==='.'?node.className.split(' ').includes(selector.slice(1)):node.tag===selector;return this.children.flatMap(child=>[...(match(child)?[child]:[]),...child.querySelectorAll(selector)]);}
}

test('three workspaces show only their own filters and preserve running scan and duplicate sessions', () => {
    const element = () => {
        const hidden = new Set();
        return { classList: { add: value => hidden.add(value), toggle: (value, active) => active ? hidden.add(value) : hidden.delete(value), contains: value => hidden.has(value) }, setAttribute() {} };
    };
    const el = Object.fromEntries(['scanControls', 'duplicateControls', 'cleanupControls', 'cleanupView', 'treePanel', 'progressToast', 'fileList', 'gridView', 'breadcrumbRow', 'emptyState'].map(key => [key, element()]));
    const buttons = Object.fromEntries(['scan', 'duplicates', 'cleanup'].map(key => ['nav-' + key, element()]));
    const state = { activeFeature: 'scan', fullTree: { path: 'C:\\' }, scanning: true, activeScanId: 3, duplicatesPhase: 'scanning', activeDuplicatesScanId: 4 };
    let rendered = 0, opened = 0, refreshed = 0;
    const context = { state, el, byId: key => buttons[key], updateScanCoverage() {}, updateBusyUi() {}, closeDropdowns() {}, setDuplicatesActive() {}, setChartsActive() {}, setSearchActive() {}, openDuplicateFinder() { opened++; }, renderCurrentLevel() { rendered++; }, window: { DiscRevealCleanup: { isCleaning: () => false, refresh: () => refreshed++ } } };
    vm.runInNewContext(fn('activateFeature'), context);
    context.activateFeature('duplicates');
    assert.equal(el.scanControls.classList.contains('hidden'), true);
    assert.equal(el.duplicateControls.classList.contains('hidden'), false);
    context.activateFeature('cleanup');
    assert.equal(el.duplicateControls.classList.contains('hidden'), true);
    assert.equal(el.cleanupView.classList.contains('hidden'), false);
    context.activateFeature('scan');
    assert.equal(el.scanControls.classList.contains('hidden'), false);
    assert.equal(el.cleanupView.classList.contains('hidden'), true);
    assert.equal(el.progressToast.classList.contains('hidden'), false);
    assert.equal(state.activeScanId, 3); assert.equal(state.activeDuplicatesScanId, 4); assert.equal(state.duplicatesPhase, 'scanning');
    assert.equal(opened, 1); assert.equal(refreshed, 1); assert.equal(rendered, 1);
});
const testDocument={body:{}, activeElement:{}, createElement:tag=>Object.assign(new TestElement(),{tag})};

test('bulk cleanup requires confirmation, retains originals and preserves changed or unsuitable copies',async()=>{
    const {oldestFile,reclaimableBytes}=duplicateHelpers();
    const keeper={path:'original',name:'original',modified:1,cleanupEligible:true};
    const good={path:'good',name:'good',modified:2,cleanupEligible:true};
    const changed={path:'changed',name:'changed',modified:3,cleanupEligible:true};
    const appFile={path:'app',name:'app',modified:4,cleanupEligible:false};
    const group={size:10,files:[keeper,good,changed,appFile]};
    const state={duplicatesPhase:'results',duplicatesCleaning:false,duplicatesResultId:9,duplicateGroups:[group],duplicateSelection:new Set(['original','good','changed','app'])};
    let allow=false,calls=[],review;
    const context={state,oldestFile,duplicatesScope:()=>({kind:'duplicates',id:9}),formatNumber:String,formatBytes:String,escapeHtml:String,plainText:String,t:key=>key,describeError:()=> 'changed',updateBusyUi(){},renderDuplicatesView(){},renderCurrentLevel(){},removeItemFromTree(){},showError(){},
        window:{DiscRevealDuplicates:{reclaimableBytes},DiscRevealModal:{confirm:async options=>{review=options;return allow;}},discreveal:{getProtection:async()=>null,deleteDuplicate:async(target,original,size,mode)=>{calls.push([target,original,size,mode]);if(target==='changed')throw {code:'duplicateChanged'};}}}};
    vm.runInNewContext(fn('cleanSelectedDuplicates'),context);
    await context.cleanSelectedDuplicates();assert.equal(calls.length,0);
    assert.equal(review.danger,true); assert.ok(review.messageHtml.includes('dup-confirm-content'));
    allow=true;await context.cleanSelectedDuplicates();
    assert.deepEqual(calls,[['good','original',10,'trash'],['changed','original',10,'trash']]);
    assert.deepEqual(group.files.map(file=>file.path),['original','changed','app']);
    assert.equal(state.duplicatesCleaning,false);
});

test('live filters combine size, filename, extension and verified name category without splitting groups',()=>{
    const {filterGroups}=duplicateHelpers();
    const group=(size,a,b)=>({size,files:[{name:a,path:'C:\\'+a,modified:1},{name:b,path:'D:\\'+b,modified:2}]});
    const groups=[group(1024**2,'report.pdf','REPORT.PDF'),group(2*1024**2,'photo.jpg','copy.jpg'),group(3*1024**2,'doc.pdf','copy.pdf')];
    const result=filterGroups(groups,{min:1,max:2,minUnit:'MB',maxUnit:'MB',types:'.JPG',name:'copy',category:'different'});
    assert.equal(result.length,1); assert.equal(result[0],groups[1]); assert.equal(result[0].files.length,2);
    assert.equal(filterGroups(groups,{category:'same'}).length,1);
    assert.equal(filterGroups(groups,{types:'png'}).length,0);
});

test('bulk suggestions and selected-byte counts preserve one original and skip unsuitable files',()=>{
    const {suggestedPaths,duplicateStats}=duplicateHelpers();
    const group={size:100,files:[{path:'original',modified:1,cleanupEligible:true},{path:'copy',modified:2,cleanupEligible:true},{path:'app',modified:3,cleanupEligible:false}]};
    assert.deepEqual([...suggestedPaths([group])],['copy']);
    const stats=duplicateStats([group],new Set(['original','copy','app']));
    assert.equal(stats.copies,2); assert.equal(stats.bytes,200); assert.equal(stats.selected,2); assert.equal(stats.selectedBytes,200);
});

test('scan options convert MB and GB, parse extensions and reject invalid ranges',()=>{
    const {extensions}=duplicateHelpers(); const context={extensions};
    vm.runInNewContext(fn('duplicateScanOptions')+'\noptions=duplicateScanOptions({min:2,minUnit:"MB",max:1,maxUnit:"GB",typeMode:"include",types:".PDF; *.jpg"});',context);
    assert.equal(context.options.minBytes,2*1024**2);assert.equal(context.options.maxBytes,1024**3);
    assert.deepEqual([...context.options.extensions],['pdf','jpg']);
    assert.throws(()=>vm.runInNewContext('duplicateScanOptions({min:2,minUnit:"GB",max:1,maxUnit:"MB",typeMode:"all"})',context));
    assert.throws(()=>vm.runInNewContext('duplicateScanOptions({typeMode:"include",types:""})',context));
});

test('duplicate scan defaults to 10 MB and the visible opt-out removes only the minimum',()=>{
    const filter = vm.runInNewContext('(' + /duplicateScanFilter: (\{[^\n]+\})/.exec(app)[1] + ')');
    const {extensions, renderDrivePicker} = duplicateHelpers(testDocument);
    const context = {extensions, filter};
    vm.runInNewContext(fn('duplicateScanOptions'), context);
    assert.equal(context.duplicateScanOptions(filter).minBytes, 10 * 1024 ** 2);
    const container = new TestElement();
    const options = {filter, onFilterChange:(key,value)=>{filter[key]=value;}, cacheEnabled:false};
    renderDrivePicker(container, [], new Set(), options);
    const checkbox = container.querySelector('.dup-size-toggle').querySelector('input');
    assert.equal(checkbox.checked, true);
    filter.max = '1'; filter.maxUnit = 'GB'; filter.typeMode = 'include'; filter.types = 'pdf';
    checkbox.checked = false; checkbox.onchange();
    const disabled = context.duplicateScanOptions(filter);
    assert.equal(disabled.minBytes, 0);
    assert.equal(disabled.maxBytes, 1024 ** 3);
    assert.deepEqual([...disabled.extensions], ['pdf']);
    assert.ok(container.querySelectorAll('.dup-min-control').every(input=>input.disabled));
    checkbox.checked = true; checkbox.onchange();
    assert.equal(context.duplicateScanOptions(filter).minBytes, 10 * 1024 ** 2);
    filter.min = '20';
    renderDrivePicker(container, [], new Set(), options);
    assert.equal(container.querySelector('.dup-size-toggle').querySelector('input').checked, true);
    assert.equal(context.duplicateScanOptions(filter).minBytes, 20 * 1024 ** 2);
});

test('shared duplicate progress preserves animation nodes, reports measured values and explains cancellation',()=>{
    const {renderProgress}=duplicateHelpers(testDocument);const container=new TestElement();
    renderProgress(container,{phase:'analyze',candidatesTotal:10,candidatesDone:3,currentPath:'C:\\test'},0,12,{onCancel(){},cancelling:true});
    const box=container.querySelector('.dup-progress');
    assert.ok(box.querySelector('.pulse-live'));
    assert.equal(box.querySelector('.progress-bar-track')['aria-valuenow'], '30');
    const fill = box.querySelector('.progress-bar-fill');
    renderProgress(container, null, 0, 13, {onCancel(){}, cancelling:true});
    assert.equal(box.querySelector('.progress-bar-fill'), fill);
    assert.equal(box.querySelector('.progress-bar-track')['aria-valuenow'], undefined);
    renderProgress(container,{phase:'analyze',candidatesTotal:10,candidatesDone:4,currentPath:'C:\\test'},0,14,{onCancel(){},cancelling:true});
    assert.equal(box.querySelector('.dup-progress-path').textContent,'C:\\test');
    assert.equal(box.querySelector('.dup-cancel-hint').textContent,'dupCancelHint');
    assert.equal(box.querySelector('.pill-btn-ghost').disabled,true);
});

test('large live results defer file rows until a group is opened',()=>{
    const {renderLiveGroups}=duplicateHelpers(testDocument); const container=new TestElement();
    const groups=Array.from({length:150},(_,index)=>({size:10,files:Array.from({length:100},(_,file)=>({path:`${index}/${file}`,name:`${file}.txt`,modified:file+1}))}));
    renderLiveGroups(container,groups,()=>{});
    assert.equal(container.querySelector('.dup-live-results').children.length,50);
    assert.equal(container.querySelectorAll('input').length,50);
    const first=container.querySelector('.dup-live-results').children[0]; first.open=true; first.ontoggle();
    assert.equal(container.querySelectorAll('input').length,75);
});
test('keeping oldest removes its prior selection and handles unknown timestamps', () => {
    const { oldestFile } = duplicateHelpers();
    const state = { duplicateSelection: new Set(['unknown', 'old', 'new']) };
    const start = app.indexOf('onKeepOnlyOldest: (group) => {');
    const end = app.indexOf('\n            },', start);
    const callback = app.slice(start + 'onKeepOnlyOldest: '.length, end) + '\n}';
    vm.runInNewContext(`(${callback})(group)`, {
        state, oldestFile, renderDuplicatesView() {},
        group: { files: [{path:'unknown',modified:0,cleanupEligible:true}, {path:'old',modified:1}, {path:'new',modified:2,cleanupEligible:true}] },
    });
    assert.deepEqual([...state.duplicateSelection].sort(), []);
});

test('live cards preserve progress, disable selection, and replace growing groups', () => {
    class Element {
        constructor() { this.children = []; this.className = ''; this.dataset = {}; }
        append(...children) { for (const child of children) this.appendChild(child); }
        appendChild(child) { child.parent = this; this.children.push(child); }
        setAttribute(name, value) { this[name]=value; }
        replaceChildren(...children) { this.children=[]; this.append(...children); }
        insertBefore(child, before) { child.parent=this; const index=this.children.indexOf(before); this.children.splice(index<0?this.children.length:index,0,child); }
        remove() { this.parent.children=this.parent.children.filter(child=>child!==this); }
        replaceWith(child) { child.parent = this.parent; this.parent.children[this.parent.children.indexOf(this)] = child; }
        querySelector(selector) { return this.children.find(child=>child.className.split(' ').includes(selector.slice(1))) || this.children.map(child=>child.querySelector(selector)).find(Boolean) || null; }
    }
    const document = { createElement:()=>new Element() };
    const { renderLiveGroups } = duplicateHelpers(document);
    const container = new Element(), progress = new Element();
    progress.className = 'dup-progress'; container.appendChild(progress);
    const pair = {size:10,files:[{path:'a',name:'a',modified:1},{path:'b',name:'b',modified:2}]};
    renderLiveGroups(container, [pair], ()=>{}, [pair]);
    const results = container.querySelector('.dup-live-results');
    const first = results.children[0];
    const descendants = node=>[node, ...node.children.flatMap(descendants)];
    assert.ok(descendants(first).filter(node=>node.type === 'checkbox').every(node=>node.disabled));
    assert.equal(first.children.find(node=>node.className === 'dup-group-header').children.find(node=>node.type === 'checkbox').disabled, true);
    renderLiveGroups(container, [pair], ()=>{}, []);
    assert.equal(results.children[0], first);
    const larger = {...pair, files:[...pair.files, {path:'c',name:'c',modified:3}]};
    renderLiveGroups(container, [larger], ()=>{}, [larger]);
    assert.equal(results.children.length, 1);
    assert.notEqual(results.children[0], first);
    assert.equal(container.children[0], progress);
    assert.equal(descendants(results).filter(node=>node.type === 'checkbox').length, 1);
    results.children[0].open=true; results.children[0].ontoggle();
    assert.equal(descendants(results).filter(node=>node.type === 'checkbox').length, 4);
});

test('streamed events update one group and ignore abandoned searches', () => {
    const state = {activeDuplicatesScanId:4,duplicateGroups:[],duplicateGroupIndex:new Map(),duplicatePendingGroups:new Map()};
    let handler;
    const start = app.indexOf('window.discreveal.onDuplicatesGroup(');
    const end = app.indexOf('window.discreveal.onDuplicatesDone', start);
    const context = {state, window:{discreveal:{onDuplicatesGroup:callback=>{handler=callback;}}},setTimeout:()=>1};
    vm.runInNewContext(fn('pathKey') + '\n' + fn('isCurrentDuplicatesScan') + '\nlet duplicatesRenderTimer=null;\n' + app.slice(start,end), context);
    const group = {size:10,files:[{path:'C:\\a'},{path:'C:\\b'}]};
    handler({scanId:3,group}); assert.equal(state.duplicateGroups.length, 0);
    handler({scanId:4,group});
    const larger = {...group,files:[...group.files,{path:'C:\\c'}]};
    handler({scanId:4,group:larger});
    assert.equal(state.duplicateGroups.length, 1);
    assert.equal(state.duplicateGroups[0].files.length, 3);
    assert.equal(state.duplicatePendingGroups.size, 1);
    state.activeDuplicatesScanId = null;
    handler({scanId:4,group});
    assert.equal(state.duplicateGroups[0].files.length, 3);
});
test('verbatim duplicate paths remove the same file from all cached trees exactly once', () => {
    const makeTree = () => ({path:'C:\\',size:20,children:[{path:'C:\\Folder',size:20,isDir:true,children:[{path:'C:\\Folder\\File',size:20}]}]});
    const current = makeTree(), cached = makeTree();
    const context = { state: { fullTree: current }, updateScanCoverage() {}, scanCache: new Map([['current',{tree:current}], ['cached',{tree:cached}]]) };
    vm.runInNewContext([fn('pathKey'), fn('findNodeChain'), fn('removeItemFromTree')].join('\n') + '\nremoveItemFromTree("\\\\\\\\?\\\\c:\\\\folder\\\\file");', context);
    assert.equal(current.size, 0); assert.equal(cached.size, 0);
    assert.equal(current.children[0].children.length, 0);
});
test('cached result actions carry the cached scan identity', () => {
    const state = {};
    const noop = () => {};
    const context = {
        state, scanCache: new Map([['C:', {tree:{path:'C:'},drive:{path:'C:'},scanId:7,settingsKey:'key'}]]),
        resetResultViews:noop, clearSearch:noop, renderCurrentLevel:noop, scanSettingsKey:()=>'key',
        adoptTree:tree=>{state.fullTree=tree;}, el:{errorBanner:{classList:{add:noop}},emptyState:{classList:{remove:noop}}},
    };
    vm.runInNewContext(fn('loadFromCacheOrReset') + '\nloadFromCacheOrReset({path:"C:"});\n' + fn('scanScope') + '\nscope=scanScope();', context);
    assert.equal(context.scope.kind, 'scan'); assert.equal(context.scope.id, 7);
});

test('late expansion cannot mutate a replaced scan or reinterpret listing pages as recursive sizes', async () => {
    const node = {path:'C:\\folder',size:10,children:[]};
    const tree = {path:'C:\\',size:30,children:[node]};
    let resolveExpansion, task, renders = 0, requestedScope;
    const context = {
        state:{fullTree:tree,resultScanId:8}, expandingOverflowFor:new Set(), expandOperationSeq:0,
        el:{fileList:{querySelector:()=>null},gridView:{querySelector:()=>null}}, currentNode:()=>null, renderCurrentLevel:()=>{renders++;},
        withErrorBanner:callback=>{task=callback();return task;},
        window:{discreveal:{expandFolder:(path,scope)=>{requestedScope=scope;return new Promise(resolve=>{resolveExpansion=resolve;});}}},
    };
    vm.runInNewContext([fn('pathKey'),fn('findNodeChain'),fn('scanScope'),fn('onExpandOverflow')].join('\n') + '\nonExpandOverflow(state.fullTree.children[0]);', context);
    context.state.fullTree = {path:'D:\\',size:99,children:[]};
    resolveExpansion([{size:17,path:'C:\\folder\\file'}]);
    await task;
    assert.equal(node.size, 10); assert.equal(tree.size, 30); assert.equal(node.children.length, 0);
    assert.equal(context.state.fullTree.size, 99); assert.equal(renders, 0);
    assert.equal(requestedScope.id, 8);
});


test('duplicate group review keeps the original and unsuitable files disabled and preserves disclosure state', () => {
    const {renderDuplicates}=duplicateHelpers(testDocument), container=new TestElement();
    const group={size:100,files:[{path:'old',name:'a.txt',modified:1,cleanupEligible:true},{path:'new',name:'a.txt',modified:2,cleanupEligible:true},{path:'system',name:'a.txt',modified:3,cleanupEligible:false}]};
    renderDuplicates(container,[group],new Set(),{});
    let card=container.querySelector('.dup-results').children[0];
    assert.equal(card.open,false); assert.equal(container.querySelectorAll('.dup-file-row').length,0);
    card.open=true; card.ontoggle();
    assert.deepEqual(card.querySelectorAll('.dup-file-row').map(row=>row.children[0].disabled),[true,false,true]);
    renderDuplicates(container,[group],new Set(['new']),{});
    card=container.querySelector('.dup-results').children[0];
    assert.equal(card.open,true); assert.equal(card.querySelector('.dup-group-header').children[0].checked,true);
});

test('duplicate file pagination can expand repeatedly without replacing the active group', () => {
    const {renderDuplicates}=duplicateHelpers(testDocument), container=new TestElement();
    const group={size:100,files:Array.from({length:80},(_,i)=>({path:String(i),name:'a.txt',modified:i+1,cleanupEligible:true}))};
    renderDuplicates(container,[group],new Set(),{});
    const card=container.querySelector('.dup-results').children[0]; card.open=true;card.ontoggle();
    const more=()=>card.querySelector('.dup-group-body').children.find(node=>node.className==='link-btn');
    assert.equal(card.querySelectorAll('.dup-file-row').length,25);more().onclick();
    assert.equal(card.querySelectorAll('.dup-file-row').length,50);more().onclick();
    assert.equal(card.querySelectorAll('.dup-file-row').length,75);more().onclick();
    assert.equal(card.querySelectorAll('.dup-file-row').length,80);assert.equal(more(),undefined);
});
