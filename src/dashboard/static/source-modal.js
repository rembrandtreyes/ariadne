// Ariadne Dashboard v2 -- Source Modal
'use strict';

class SourceModal {
    static _open = false;

    static open(sourceData) {
        if (!sourceData) return;

        SourceModal._open = true;

        const modal = document.getElementById('source-modal');
        const header = document.getElementById('source-modal-header');
        const codeEl = document.getElementById('source-modal-code');
        if (!modal || !header || !codeEl) return;

        SourceModal.render(sourceData);

        modal.classList.remove('hidden');
    }

    static render(sourceData) {
        const header = document.getElementById('source-modal-header');
        const codeEl = document.getElementById('source-modal-code');
        if (!header || !codeEl) return;

        const lineRange = sourceData.line_start && sourceData.line_end
            ? ` (L${esc(String(sourceData.line_start))}-${esc(String(sourceData.line_end))})`
            : '';
        header.innerHTML = `
            <span class="source-modal__path">${esc(sourceData.file || '')}${lineRange}</span>
            <button class="source-modal__close" onclick="SourceModal.close()">&times;</button>
        `;

        const lines = (sourceData.code || '').split('\n');
        const startLine = sourceData.line_start || 1;
        const language = sourceData.language || '';

        let codeHtml = '';
        for (let i = 0; i < lines.length; i++) {
            const lineNum = startLine + i;
            const highlighted = DetailPanel.highlightSyntax(lines[i], language);
            codeHtml += `<div><span class="detail-panel__code-line-num">${esc(String(lineNum))}</span>${highlighted}</div>`;
        }

        codeEl.innerHTML = codeHtml;
    }

    static close() {
        SourceModal._open = false;
        const modal = document.getElementById('source-modal');
        if (modal) modal.classList.add('hidden');
    }

    static isOpen() {
        return SourceModal._open;
    }
}
