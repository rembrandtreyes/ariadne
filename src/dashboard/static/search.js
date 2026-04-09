// Ariadne Dashboard v2 -- Search
'use strict';

class Search {
    static _debounceTimer = null;
    static _results = [];
    static _selectedIndex = -1;
    static _dropdownEl = null;
    static _isOpen = false;

    static init() {
        const input = document.getElementById('search-input');
        if (!input) return;

        input.addEventListener('input', () => {
            clearTimeout(Search._debounceTimer);
            const term = input.value.trim();
            if (term.length < 2) {
                Search.close();
                return;
            }
            Search._debounceTimer = setTimeout(() => Search.query(term), 200);
        });

        input.addEventListener('keydown', (e) => {
            if (!Search._isOpen) return;
            if (e.key === 'ArrowDown') {
                e.preventDefault();
                Search._selectedIndex = Math.min(Search._selectedIndex + 1, Search._results.length - 1);
                Search._highlightSelected();
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                Search._selectedIndex = Math.max(Search._selectedIndex - 1, 0);
                Search._highlightSelected();
            } else if (e.key === 'Enter' && Search._selectedIndex >= 0) {
                e.preventDefault();
                const r = Search._results[Search._selectedIndex];
                if (r) Search.selectResult(r);
            } else if (e.key === 'Escape') {
                Search.close();
            }
        });

        input.addEventListener('focus', () => {
            if (input.value.trim().length >= 2) {
                Search.query(input.value.trim());
            }
        });
    }

    static focus() {
        const input = document.getElementById('search-input');
        if (input) input.focus();
    }

    static isOpen() {
        return Search._isOpen;
    }

    static close() {
        Search._isOpen = false;
        Search._results = [];
        Search._selectedIndex = -1;
        if (Search._dropdownEl) {
            Search._dropdownEl.remove();
            Search._dropdownEl = null;
        }
    }

    static async query(term) {
        try {
            const res = await fetch(`/api/search?q=${encodeURIComponent(term)}`);
            if (!res.ok) return;
            const results = await res.json();
            Search._results = results.slice(0, 10);
            Search._selectedIndex = -1;
            Search.renderResults(Search._results);
        } catch (e) {
            console.error('Search error:', e);
        }
    }

    static renderResults(results) {
        Search.close();
        if (results.length === 0) return;

        Search._isOpen = true;
        Search._results = results;

        const container = document.getElementById('search-container');
        const dropdown = document.createElement('div');
        dropdown.className = 'search-dropdown';

        let html = '';
        for (let i = 0; i < results.length; i++) {
            const r = results[i];
            const fileName = r.file ? r.file.split('/').pop() : '';
            html += `<div class="search-result" data-index="${esc(String(i))}" onclick="Search._selectByIndex(${i})">
                <span class="search-result__name">${esc(r.name)}</span>
                <span class="search-result__kind">${esc(r.kind)}</span>
                <span class="search-result__file">${esc(fileName)}</span>
            </div>`;
        }
        html += `<div class="search-hint">
            <span><kbd>&#8593;&#8595;</kbd> navigate</span>
            <span><kbd>Enter</kbd> select</span>
            <span><kbd>Esc</kbd> close</span>
        </div>`;

        dropdown.innerHTML = html;
        container.appendChild(dropdown);
        Search._dropdownEl = dropdown;
    }

    static _highlightSelected() {
        if (!Search._dropdownEl) return;
        const items = Search._dropdownEl.querySelectorAll('.search-result');
        items.forEach((el, i) => {
            el.classList.toggle('search-result--selected', i === Search._selectedIndex);
        });
    }

    static _selectByIndex(index) {
        const r = Search._results[index];
        if (r) Search.selectResult(r);
    }

    // result: object with shape { id: number|string, name: string, kind: string, file: string }
    // Extracts moduleName from result.file and calls App.drillDown(moduleName, symbolId).
    static selectResult(result) {
        Search.close();
        document.getElementById('search-input').value = '';

        // Extract module name from file path
        const filePath = result.file || '';
        const pathWithoutSrc = filePath.startsWith('src/') ? filePath.slice(4) : filePath;
        const moduleName = pathWithoutSrc.includes('/') ? pathWithoutSrc.split('/')[0] : 'root';

        const symbolId = parseInt(result.id, 10);
        if (typeof App !== 'undefined' && App.drillDown) {
            App.drillDown(moduleName, symbolId);
        }
    }
}
