const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname, '..');
const benchmark = require('../scripts/duplicateBenchmark');
const median = values => [...values].sort((a,b)=>a-b)[Math.floor(values.length/2)];

test('duplicate comparison preserves measured medians and the losing case', () => {
    const data=benchmark.data;
    assert.equal(data.workloads.length,4);
    assert.match(data.scope,/Neither graphical interface included/);
    assert.match(data.cache,/application caches disabled/);
    for(const workload of data.workloads) {
        for(const engine of ['discReveal','czkawka']) {
            assert.equal(workload.times[engine].length,5);
            assert.equal(median(workload.times[engine]),workload.medians[engine]);
        }
        assert.match(workload.correctness,/all and only/);
    }
    assert.equal(data.workloads.filter(w=>w.medians.discReveal<w.medians.czkawka).length,3);
    const collision=data.workloads.find(w=>w.name==='sample-collisions');
    assert.ok(collision.medians.discReveal>collision.medians.czkawka);
});

test('both public languages disclose benchmark scope and match the source numbers', () => {
    for(const lang of ['en','de']) {
        const prefix=lang==='de'?'de/':'';
        const html=fs.readFileSync(path.join(root,'docs',prefix,'benchmarks/index.html'),'utf8');
        const text=fs.readFileSync(path.join(root,'docs',prefix,'benchmarks/index.md'),'utf8');
        assert.ok(html.includes(benchmark.copy[lang].scope));
        assert.ok(text.includes(benchmark.copy[lang].scope));
        assert.ok(html.includes(benchmark.copy[lang].limits));
        assert.match(html,/id="duplicate-benchmark"/);
        assert.match(html,/benchmarks\/duplicates-0\.2\.5\.json/);
        assert.match(html,/href="\/style\.css\?v=[a-f0-9]{12}"/);
        for(const work of benchmark.data.workloads) {
            for(const value of Object.values(work.medians)) {
                assert.ok(html.includes(Math.round(value).toLocaleString(lang==='de'?'de-DE':'en-US')+' ms'));
            }
        }
    }
    assert.match(fs.readFileSync(path.join(root,'docs/llms.txt'),'utf8'),/Duplicate-search core comparison/);
});

test('homepage keeps two app guides and a compact measured scan comparison without detailed duplicate panels', () => {
    for (const lang of ['en','de']) {
        const prefix=lang==='de'?'de/':'', html=fs.readFileSync(path.join(root,'docs',prefix,'index.html'),'utf8');
        const guides=html.match(/<section id="guides"[\s\S]*?<\/section>/)[0];
        assert.equal((guides.match(/class="guide-card"/g)||[]).length,2);
        for (const id of ['disk-space','duplicate-files']) {
            const article=require('../docs-src/guides').find(a=>a.id===id);
            assert.ok(guides.includes('/'+article[lang].slug+'/'));
        }
        assert.ok(!html.includes('dupe-bench-panel'));
        assert.ok(html.includes('bench-chart'));
        assert.ok(html.includes(lang==='de'?'1,19 s':'1.19 s'));
        assert.ok(!html.includes('bench-preview-duplicate'));
        assert.ok(!html.includes(lang==='de'?'3 von 4':'3 of 4'));
        assert.ok(!html.includes('bench-preview-note'));
        assert.ok(!html.includes(lang==='de'?'warmer Windows-Dateicache':'warm Windows file cache'));
        for (const anchor of ['drive-scan','duplicate-benchmark']) {
            assert.ok(html.includes(`href="${require('../scripts/buildBenchmarkPages').route(lang,anchor)}"`));
            const report=fs.readFileSync(path.join(root,'docs',prefix,'benchmarks/index.html'),'utf8');
            assert.ok(report.includes(`id="${anchor}"`));
            assert.ok(!report.includes('Neu in 0.2.5'));
            assert.ok(!report.includes('New in 0.2.5'));
            assert.ok(!report.includes('1.99 to 1.87'));
            assert.ok(!report.includes('1,99 auf 1,87'));
        }
    }
});
