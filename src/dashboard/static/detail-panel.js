// Ariadne Dashboard v2 -- Detail Panel
'use strict';

class DetailPanel {
    static _open = false;
    static _currentSymbolId = null;
    static _lastSource = null;

    static async open(symbolId) {
        DetailPanel._currentSymbolId = symbolId;
        DetailPanel._open = true;

        const panel = document.getElementById('detail-panel');
        const content = document.getElementById('detail-content');
        const header = document.getElementById('detail-header');
        if (!panel || !content || !header) return;

        content.innerHTML = '<div style="padding: 24px; color: var(--text-muted);">Loading...</div>';
        panel.classList.add('detail-panel--open');

        try {
            const data = await DetailPanel.fetchData(symbolId);
            DetailPanel.renderHeader(header, data.selfNode, data.describe);
            content.innerHTML =
                DetailPanel.renderDescription(data.describe) +
                DetailPanel.renderSource(data.source) +
                DetailPanel.renderRiskFactors(data.describe ? data.describe.metrics : null) +
                DetailPanel.renderBlastRadius(data.describe ? data.describe.metrics : null) +
                DetailPanel.renderCallers(data.callers) +
                DetailPanel.renderCallees(data.callees) +
                DetailPanel.renderIssues(data.selfNode, data.describe);
        } catch (e) {
            console.error('DetailPanel error:', e);
            content.innerHTML = '<div style="padding: 24px; color: var(--text-muted);">Failed to load details.</div>';
        }
    }

    static close() {
        DetailPanel._open = false;
        DetailPanel._currentSymbolId = null;
        const panel = document.getElementById('detail-panel');
        if (panel) panel.classList.remove('detail-panel--open');
    }

    static isOpen() {
        return DetailPanel._open;
    }

    static async fetchData(symbolId) {
        const [descRes, sourceRes, neighborRes] = await Promise.all([
            fetch(`/api/describe?id=${symbolId}`),
            fetch(`/api/source?id=${symbolId}&context=0`),
            fetch(`/api/graph/neighborhood?id=${symbolId}&depth=1`),
        ]);

        const describe = descRes.ok ? await descRes.json() : null;
        const source = sourceRes.ok ? await sourceRes.json() : null;
        const neighborhood = neighborRes.ok ? await neighborRes.json() : null;

        let callers = [];
        let callees = [];
        if (neighborhood) {
            const selfId = String(symbolId);
            for (const edge of (neighborhood.edges || [])) {
                if (String(edge.target) === selfId) {
                    const node = (neighborhood.nodes || []).find(n => String(n.id) === String(edge.source));
                    if (node) callers.push(node);
                } else if (String(edge.source) === selfId) {
                    const node = (neighborhood.nodes || []).find(n => String(n.id) === String(edge.target));
                    if (node) callees.push(node);
                }
            }
        }

        const selfNode = neighborhood
            ? (neighborhood.nodes || []).find(n => String(n.id) === String(symbolId))
            : null;

        return { describe, source, callers, callees, selfNode };
    }

    static renderHeader(headerEl, selfNode, describe) {
        const name = selfNode ? selfNode.name : (describe ? describe.role : 'Symbol');
        const file = selfNode ? selfNode.file : '';
        headerEl.innerHTML = `
            <div>
                <div class="detail-panel__name">${esc(name)}</div>
                <div class="detail-panel__file">${esc(file)}</div>
            </div>
            <button class="detail-panel__close" onclick="DetailPanel.close()">&times;</button>
        `;
    }

    static renderDescription(describe) {
        if (!describe) return '';
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Description</div>
            <div class="detail-panel__description">${esc(describe.description)}</div>
        </div>`;
    }

    static renderSource(source) {
        if (!source || !source.code) return '';

        const lines = source.code.split('\n');
        const lineCount = source.line_count || lines.length;
        const showLines = lineCount < 25 ? lines : lines.slice(0, 15);
        const startLine = source.line_start || 1;

        let codeHtml = '';
        for (let i = 0; i < showLines.length; i++) {
            const lineNum = startLine + i;
            const highlighted = DetailPanel.highlightSyntax(showLines[i], source.language || '');
            codeHtml += `<div><span class="detail-panel__code-line-num">${esc(String(lineNum))}</span>${highlighted}</div>`;
        }

        let viewMore = '';
        if (lineCount >= 25) {
            viewMore = `<button class="detail-panel__view-source" onclick="SourceModal.open(DetailPanel._lastSource)">View full source (${esc(String(lineCount))} lines)</button>`;
        }

        DetailPanel._lastSource = source;

        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Source</div>
            <div class="detail-panel__code">${codeHtml}</div>
            ${viewMore}
        </div>`;
    }

    static renderCallers(callers) {
        if (!callers || callers.length === 0) return '';
        let callerHtml = '';
        for (const c of callers.slice(0, 10)) {
            callerHtml += `<li class="detail-panel__symbol-item" onclick="DetailPanel.open(${esc(String(c.id))})">${esc(c.name)} <span style="color:var(--text-muted)">${esc(c.kind)}</span></li>`;
        }
        if (callers.length > 10) {
            callerHtml += `<li class="detail-panel__symbol-item" style="color:var(--text-muted)">... and ${esc(String(callers.length - 10))} more</li>`;
        }
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Called By (${esc(String(callers.length))})</div>
            <ul class="detail-panel__symbol-list">${callerHtml}</ul>
        </div>`;
    }

    static renderCallees(callees) {
        if (!callees || callees.length === 0) return '';
        let calleeHtml = '';
        for (const c of callees.slice(0, 10)) {
            calleeHtml += `<li class="detail-panel__symbol-item" onclick="DetailPanel.open(${esc(String(c.id))})">${esc(c.name)} <span style="color:var(--text-muted)">${esc(c.kind)}</span></li>`;
        }
        if (callees.length > 10) {
            calleeHtml += `<li class="detail-panel__symbol-item" style="color:var(--text-muted)">... and ${esc(String(callees.length - 10))} more</li>`;
        }
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Depends On (${esc(String(callees.length))})</div>
            <ul class="detail-panel__symbol-list">${calleeHtml}</ul>
        </div>`;
    }

    static renderRiskFactors(metrics) {
        if (!metrics) return '';
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Risk Factors</div>
            ${DetailPanel._renderRiskBar('Fan In', metrics.fan_in, 20)}
            ${DetailPanel._renderRiskBar('Fan Out', metrics.fan_out, 20)}
            ${DetailPanel._renderRiskBar('Churn', metrics.modification_count, 30)}
            ${DetailPanel._renderRiskBar('Coupling', Math.round(metrics.max_coupling_strength * 100), 100)}
        </div>`;
    }

    static renderBlastRadius(metrics) {
        if (!metrics || metrics.blast_radius === 0) return '';
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Blast Radius</div>
            <div style="font-size:13px;color:var(--text-muted)">Changing this symbol could affect <strong>${esc(String(metrics.blast_radius))}</strong> downstream symbols.</div>
        </div>`;
    }

    static renderIssues(selfNode, describe) {
        if (!describe) return '';
        const riskLevel = describe.risk_level;
        const riskScore = describe.risk_score;
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Assessment</div>
            <div style="display:flex;align-items:center;gap:8px;">
                <span class="risk-card__badge risk-card__badge--${esc(riskLevel)}">${esc(riskLevel)}</span>
                <span style="font-size:13px;color:var(--text-muted)">Risk score: ${esc(String(Math.round(riskScore * 100)))}%</span>
            </div>
        </div>`;
    }

    static highlightSyntax(line, language) {
        // Call esc() first to prevent XSS, then apply highlighting spans
        let result = esc(line);

        // Comments (run first to avoid re-highlighting inside comments)
        result = result.replace(/(\/\/.*$)/gm, '<span class="syn-comment">$1</span>');

        // Strings (esc() converts " to &quot; and ' to &#39;)
        result = result.replace(/(&quot;[^&]*?&quot;)/g, '<span class="syn-string">$1</span>');
        result = result.replace(/(&#39;[^&]*?&#39;)/g, '<span class="syn-string">$1</span>');

        // Numbers
        result = result.replace(/\b(\d+\.?\d*)\b/g, '<span class="syn-number">$1</span>');

        // Language keywords
        const rustKeywords = /\b(fn|let|mut|const|pub|use|mod|struct|enum|impl|trait|where|self|Self|return|if|else|match|for|while|loop|break|continue|async|await|move|dyn|Box|Vec|Option|Result|Some|None|Ok|Err|true|false)\b/g;
        const jsKeywords = /\b(function|const|let|var|return|if|else|for|while|class|static|async|await|new|this|import|export|default|try|catch|throw|typeof|instanceof|true|false|null|undefined)\b/g;
        const pyKeywords = /\b(def|class|return|if|elif|else|for|while|import|from|as|with|try|except|raise|True|False|None|self|lambda|yield|async|await|pass|break|continue)\b/g;

        const lang = (language || '').toLowerCase();
        let keywords = rustKeywords;
        if (lang === 'javascript' || lang === 'typescript' || lang === 'js' || lang === 'ts') {
            keywords = jsKeywords;
        } else if (lang === 'python' || lang === 'py') {
            keywords = pyKeywords;
        }

        result = result.replace(keywords, '<span class="syn-keyword">$1</span>');

        return result;
    }

    static _renderRiskBar(label, value, maxVal) {
        const pct = Math.min(Math.round((value / maxVal) * 100), 100);
        const color = pct >= 80 ? 'var(--health-red)' : pct >= 50 ? 'var(--health-orange)' : pct >= 25 ? 'var(--health-yellow)' : 'var(--health-green)';
        return `<div class="detail-panel__risk-bar">
            <span class="detail-panel__risk-bar-label">${esc(label)}</span>
            <div class="detail-panel__risk-bar-track">
                <div class="detail-panel__risk-bar-fill" style="width:${esc(String(pct))}%;background:${esc(color)}"></div>
            </div>
            <span class="detail-panel__risk-bar-value">${esc(String(value))}</span>
        </div>`;
    }
}
