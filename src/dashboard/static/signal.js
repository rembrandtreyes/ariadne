// Ariadne Dashboard v2 -- Signal View
'use strict';

class Signal {
    static _scrollY = 0;
    static _data = null;

    static async init() {
        try {
            Signal._data = await Signal.fetchData();
            Signal.renderHero(Signal._data.stats, Signal._data.insights);
            Signal.renderTopStats(Signal._data.stats);
            Signal.renderRisks(Signal._data.insights, Signal._data.modules);
            Signal.renderModules(Signal._data.modules);
            Signal.renderCoupling(Signal._data.coupling);
            Signal.renderDeadCode(Signal._data.insights);
        } catch (e) {
            console.error('Signal init error:', e);
        }
    }

    static async fetchData() {
        const [statsRes, modulesRes, insightsRes, couplingRes] = await Promise.all([
            fetch('/api/stats'),
            fetch('/api/modules'),
            fetch('/api/graph/insights'),
            fetch('/api/coupling?limit=10'),
        ]);

        const stats = await statsRes.json();
        const modulesData = await modulesRes.json();
        const insights = await insightsRes.json();
        const couplingData = await couplingRes.json();

        return {
            stats,
            modules: modulesData.modules || [],
            insights,
            coupling: couplingData.pairs || [],
        };
    }

    // Computes health score 0-100 using the weighted formula:
    //   resolution_rate(30%) + (1-dead_ratio)(25%) + (1-cycle_score)(20%) + (1-god_score)(15%) + coupling_health(10%)
    // coupling_health is fixed at 0.8 (no per-file coupling health available from /api/coupling).
    static computeHealthScore(stats, insights) {
        const resolutionRate = stats.resolution_rate || 0;
        const deadRatio = stats.symbols > 0 ? (stats.dead_functions || 0) / stats.symbols : 0;
        const cycleScore = Math.min((insights.circular_deps || []).length * 0.05, 1.0);
        const godScore = Math.min((insights.god_files || []).length * 0.10, 1.0);
        const couplingHealth = 0.8;

        const raw =
            resolutionRate    * 0.30 +
            (1 - deadRatio)   * 0.25 +
            (1 - cycleScore)  * 0.20 +
            (1 - godScore)    * 0.15 +
            couplingHealth    * 0.10;

        return Math.max(0, Math.min(100, Math.round(raw * 100)));
    }

    static _healthColor(score) {
        if (score >= 80) return 'var(--health-green)';
        if (score >= 60) return 'var(--health-yellow)';
        if (score >= 40) return 'var(--health-orange)';
        return 'var(--health-red)';
    }

    static _healthLabel(score) {
        if (score >= 80) return 'Healthy';
        if (score >= 60) return 'Moderate';
        if (score >= 40) return 'At Risk';
        return 'Critical';
    }

    // stats: object from /api/stats — fields: symbols(number), files(number), calls(number), dead_functions(number), resolution_rate(float 0-1), languages(string[])
    // insights: object from /api/graph/insights — fields: circular_deps(array), god_files(array), most_connected(array), dead_code(array)
    static renderHero(stats, insights) {
        const score = Signal.computeHealthScore(stats, insights);
        const color = Signal._healthColor(score);
        const label = Signal._healthLabel(score);

        const el = document.getElementById('signal-hero');
        if (!el) return;

        el.innerHTML = `
            <div class="signal-hero__score" style="color: ${esc(color)}">${esc(String(score))}</div>
            <div class="signal-hero__label">${esc(label)}</div>
            <div class="signal-hero__summary">
                ${esc(String(stats.symbols || 0))} symbols across ${esc(String(stats.files || 0))} files in ${esc(String((stats.languages || []).length))} languages.
                ${esc(String(stats.dead_functions || 0))} unreachable symbols detected.
            </div>
            <div class="signal-hero__stats">
                <div><span class="signal-hero__stat-value">${esc(String(stats.files || 0))}</span> files</div>
                <div><span class="signal-hero__stat-value">${esc(String(stats.symbols || 0))}</span> symbols</div>
                <div><span class="signal-hero__stat-value">${esc(String(stats.calls || 0))}</span> calls</div>
                <div><span class="signal-hero__stat-value">${esc(String(Math.round((stats.resolution_rate || 0) * 100)))}%</span> resolved</div>
            </div>
        `;
    }

    // stats: object from /api/stats — fields: files(number), symbols(number), languages(string[])
    static renderTopStats(stats) {
        const el = document.getElementById('top-stats');
        if (!el) return;
        el.innerHTML = `
            <span><span class="top-bar__stat-value">${esc(String(stats.files || 0))}</span> files</span>
            <span><span class="top-bar__stat-value">${esc(String(stats.symbols || 0))}</span> symbols</span>
            <span><span class="top-bar__stat-value">${esc(String((stats.languages || []).length))}</span> langs</span>
        `;
    }

    // insights: object from /api/graph/insights — uses insights.most_connected (top 5 by connection count) as risk candidates
    // modules: array from /api/modules (unused in this method, reserved for future use)
    // Fetches narrative descriptions for each candidate from /api/describe?id=<id> (defined in PRD-01).
    static async renderRisks(insights, modules) {
        const container = document.getElementById('risk-cards');
        if (!container) return;

        const candidates = (insights.most_connected || []).slice(0, 5);
        if (candidates.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No significant risks detected.</div>';
            return;
        }

        const descPromises = candidates.map(async (c) => {
            try {
                const res = await fetch(`/api/describe?id=${encodeURIComponent(c.id)}`);
                if (res.ok) return await res.json();
            } catch (_) {}
            return null;
        });

        const descriptions = await Promise.all(descPromises);

        let html = '';
        for (let i = 0; i < candidates.length; i++) {
            const c = candidates[i];
            const desc = descriptions[i];
            const riskLevel = desc ? desc.risk_level : 'low';
            const description = desc
                ? esc(desc.description)
                : `${esc(c.name)} has ${esc(String(c.connections))} connections.`;

            const filePath = c.file || '';
            const pathWithoutSrc = filePath.startsWith('src/') ? filePath.slice(4) : filePath;
            const moduleName = pathWithoutSrc.includes('/') ? pathWithoutSrc.split('/')[0] : 'root';

            html += `<div class="risk-card" onclick="App.drillDown('${esc(moduleName)}', ${esc(String(c.id))})">
                <div class="risk-card__header">
                    <span class="risk-card__name">${esc(c.name)}</span>
                    <span class="risk-card__badge risk-card__badge--${esc(riskLevel)}">${esc(riskLevel)}</span>
                </div>
                <div class="risk-card__description">${description}</div>
            </div>`;
        }

        container.innerHTML = html;
    }

    // modules: array of module objects from /api/modules — each has fields: name(string), symbol_count(number), file_count(number), dead_count(number), health(float 0-1), files(array of {health: float})
    static renderModules(modules) {
        const container = document.getElementById('module-grid');
        if (!container) return;

        if (!modules || modules.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No modules found. Run ariadne index first.</div>';
            return;
        }

        let html = '';
        for (const m of modules) {
            const healthPct = Math.round((m.health || 0) * 100);
            const healthColor = Signal._healthColor(healthPct);

            let sparkline = '';
            const files = (m.files || []).slice(0, 20);
            for (const f of files) {
                const h = Math.round((f.health || 0) * 100);
                const barH = Math.max(3, Math.round(h / 100 * 20));
                const color = Signal._healthColor(h);
                sparkline += `<div class="module-card__file-bar" style="height:${esc(String(barH))}px;background:${esc(color)}"></div>`;
            }

            html += `<div class="module-card" onclick="App.drillDown('${esc(m.name)}')">
                <div class="module-card__name">${esc(m.name)}</div>
                <div class="module-card__stats">
                    <span>${esc(String(m.symbol_count))} symbols</span>
                    <span>${esc(String(m.file_count))} files</span>
                    <span>${esc(String(m.dead_count))} dead</span>
                </div>
                <div class="module-card__health-bar">
                    <div class="module-card__health-fill" style="width:${esc(String(healthPct))}%;background:${esc(healthColor)}"></div>
                </div>
                <div class="module-card__files">${sparkline}</div>
            </div>`;
        }

        container.innerHTML = html;
    }

    // coupling: array of pair objects from /api/coupling — each has fields: from_file(string), to_file(string), from_module(string), strength(float 0-1)
    static renderCoupling(coupling) {
        const container = document.getElementById('coupling-list');
        if (!container) return;

        if (!coupling || coupling.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No coupling data available.</div>';
            return;
        }

        let html = '';
        for (const c of coupling) {
            const strengthPct = Math.round((c.strength || 0) * 100);
            const color = c.strength >= 0.7 ? 'var(--health-red)' : c.strength >= 0.4 ? 'var(--health-orange)' : 'var(--health-yellow)';
            const fromFile = (c.from_file || '').split('/').pop() || c.from_file;
            const toFile = (c.to_file || '').split('/').pop() || c.to_file;

            html += `<div class="coupling-row" onclick="App.drillDown('${esc(c.from_module)}')">
                <div class="coupling-row__files">
                    <span>${esc(fromFile)}</span>
                    <span class="coupling-row__arrow">&#8596;</span>
                    <span>${esc(toFile)}</span>
                </div>
                <span class="coupling-row__strength" style="color:${esc(color)}">${esc(String(strengthPct))}%</span>
                <div class="coupling-row__bar">
                    <div class="coupling-row__bar-fill" style="width:${esc(String(strengthPct))}%;background:${esc(color)}"></div>
                </div>
            </div>`;
        }

        container.innerHTML = html;
    }

    // insights: object from /api/graph/insights — uses insights.dead_code (array of {id: number, name: string, file: string})
    static renderDeadCode(insights) {
        const container = document.getElementById('dead-code-grid');
        if (!container) return;

        const deadCode = (insights.dead_code || []).slice(0, 20);
        if (deadCode.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No dead code detected.</div>';
            return;
        }

        let html = '';
        for (const d of deadCode) {
            const fileName = (d.file || '').split('/').pop() || d.file;
            const filePath = d.file || '';
            const pathWithoutSrc = filePath.startsWith('src/') ? filePath.slice(4) : filePath;
            const moduleName = pathWithoutSrc.includes('/') ? pathWithoutSrc.split('/')[0] : 'root';

            html += `<div class="dead-code-item" onclick="App.drillDown('${esc(moduleName)}', ${esc(String(d.id))})">
                <span class="dead-code-item__icon">&#9679;</span>
                <span class="dead-code-item__name">${esc(d.name)}</span>
                <span class="dead-code-item__file">${esc(fileName)}</span>
            </div>`;
        }

        container.innerHTML = html;
    }

    static show() {
        const el = document.getElementById('signal-view');
        if (el) {
            el.classList.remove('hidden', 'fade-out');
            el.classList.add('fade-in');
        }
    }

    static hide() {
        const el = document.getElementById('signal-view');
        if (el) {
            el.classList.add('fade-out');
            setTimeout(() => el.classList.add('hidden'), 250);
        }
    }

    static saveScrollPosition() {
        Signal._scrollY = window.scrollY;
    }

    static restoreScrollPosition() {
        window.scrollTo(0, Signal._scrollY);
    }
}
