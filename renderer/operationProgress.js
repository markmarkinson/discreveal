/* discReveal by markMarkinson: one progress language, based on the drive scan. */
(function () {
    'use strict';
    function element(tag, className) {
        const node = document.createElement(tag); node.className = className; return node;
    }
    function create(className, onCancel) {
        const box = element('div', 'operation-progress ' + className); box.setAttribute('role', 'status');
        const header = element('div', 'progress-toast-header');
        const dot = element('span', 'pulse-live'); dot.setAttribute('aria-hidden', 'true');
        const status = element('span', 'operation-progress-status dup-progress-status');
        const elapsed = element('span', 'operation-progress-elapsed dup-progress-elapsed'); elapsed.setAttribute('aria-live', 'off');
        const cancel = element('button', 'pill-btn pill-btn-ghost operation-progress-cancel'); cancel.type = 'button'; cancel.onclick = onCancel;
        header.append(dot, status, elapsed, cancel);
        const track = element('div', 'progress-bar-track'); track.setAttribute('role', 'progressbar');
        track.setAttribute('aria-valuemin', '0'); track.setAttribute('aria-valuemax', '100');
        track.appendChild(element('div', 'progress-bar-fill'));
        const detail = element('div', 'progress-detail operation-progress-detail dup-progress-path');
        const hint = element('div', 'operation-progress-hint dup-cancel-hint'); hint.hidden = true;
        box.append(header, track, detail, hint); return box;
    }
    function update(box, { status, detail = '', title = '', elapsed = '', percentage = null, cancelling = false, cancelVisible = true, hint = '' }) {
        const assign = (selector, text) => { const node = box.querySelector(selector); if (node.textContent !== text) node.textContent = text; return node; };
        assign('.operation-progress-status', status);
        assign('.operation-progress-elapsed', elapsed);
        const path = assign('.operation-progress-detail', detail); path.hidden = !detail; path.title = title || detail;
        const cancel = assign('.operation-progress-cancel', window.DiscRevealI18n.t('cancel')); cancel.hidden = !cancelVisible; cancel.disabled = cancelling;
        const note = assign('.operation-progress-hint', hint); note.hidden = !hint;
        const fill = box.querySelector('.progress-bar-fill');
        const track = box.querySelector('.progress-bar-track');
        const determinate = Number.isFinite(percentage);
        fill.className = 'progress-bar-fill' + (determinate ? '' : ' is-indeterminate');
        fill.style.width = determinate ? Math.max(0, Math.min(100, percentage)) + '%' : '';
        if (determinate) track.setAttribute('aria-valuenow', String(Math.max(0, Math.min(100, percentage)))); else track.removeAttribute('aria-valuenow');
        track.setAttribute('aria-label', status);
        box.setAttribute('aria-busy', 'true');
    }
    window.DiscRevealProgress = Object.freeze({ create, update });
})();
