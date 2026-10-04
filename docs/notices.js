(() => {
    'use strict';
    const search = document.getElementById('notices-search');
    const rows = [...document.querySelectorAll('.notices-table-wrap tbody tr')];
    const count = document.querySelector('.notices-count');
    search.addEventListener('input', () => {
        const query = search.value.trim().toLowerCase();
        let visible = 0;
        for (const row of rows) {
            row.hidden = !row.textContent.toLowerCase().includes(query);
            if (!row.hidden) visible++;
        }
        count.textContent = query ? `${visible} / ${rows.length}` : String(rows.length);
        document.getElementById('notices-empty').hidden = visible !== 0;
    });
})();
