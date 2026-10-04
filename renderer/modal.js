/**
 * Modal confirmation dialog.
 */
(function () {
    'use strict';

    const { escapeHtml } = window.DiscRevealUtil;
    const { t } = window.DiscRevealI18n;

    /**
     * Shows a confirmation dialog and resolves to `true` if confirmed, `false`
     * if dismissed (Cancel, Escape or a click on the backdrop).
     *
     * `options.messageHtml` is inserted as HTML; callers must escape any
     * dynamic values in it. With `options.danger`, initial focus is on Cancel
     * so a stray Enter cannot confirm a destructive action. With
     * `options.acknowledgement` (a text), the confirm button stays disabled
     * until the user ticks a checkbox with that text. Keyboard focus stays
     * inside the dialog and returns to the previous element on close.
     */
    function confirm(options) {
        return new Promise((resolve) => {
            const previouslyFocused = document.activeElement;

            const overlay = document.createElement('div');
            overlay.className = 'modal-overlay';

            const box = document.createElement('div');
            box.className = `modal-box${options.danger ? ' danger' : ''}`;
            box.setAttribute('role', 'dialog');
            box.setAttribute('aria-modal', 'true');
            box.setAttribute('aria-labelledby', 'modal-title');
            box.innerHTML = `
                <div class="modal-title" id="modal-title">${escapeHtml(options.title)}</div>
                <div class="modal-message" tabindex="0">${options.messageHtml}</div>
                ${
                    options.acknowledgement
                        ? `<label class="modal-acknowledge"><input type="checkbox" data-action="acknowledge" /><span>${escapeHtml(options.acknowledgement)}</span></label>`
                        : ''
                }
                <div class="modal-actions">
                    <button type="button" class="pill-btn pill-btn-ghost" data-action="cancel">${escapeHtml(t('cancel'))}</button>
                    <button type="button" class="pill-btn ${options.danger ? 'pill-btn-danger' : 'pill-btn-primary'}" data-action="confirm">${escapeHtml(options.confirmLabel || t('confirmDefault'))}</button>
                </div>
            `;

            overlay.appendChild(box);
            document.body.appendChild(overlay);

            const cancelButton = box.querySelector('[data-action="cancel"]');
            const confirmButton = box.querySelector('[data-action="confirm"]');
            const acknowledgeBox = box.querySelector('[data-action="acknowledge"]');

            if (acknowledgeBox) {
                confirmButton.disabled = true;
                acknowledgeBox.onchange = () => {
                    confirmButton.disabled = !acknowledgeBox.checked;
                };
            }

            const focusOrder = () => [box.querySelector('.modal-message'), acknowledgeBox, cancelButton, confirmButton].filter((element) => element && !element.disabled);

            const close = (result) => {
                overlay.remove();
                document.removeEventListener('keydown', onKeydown, true);
                if (previouslyFocused?.isConnected) previouslyFocused.focus();
                resolve(result);
            };
            const onKeydown = (event) => {
                if (event.key === 'Escape') {
                    event.preventDefault();
                    close(false);
                } else if (event.key === 'Tab') {
                    // Keep Tab cycling between the dialog's own controls.
                    event.preventDefault();
                    const order = focusOrder();
                    const step = event.shiftKey ? -1 : 1;
                    const index = order.indexOf(document.activeElement);
                    order[(index + step + order.length) % order.length].focus();
                }
            };

            overlay.addEventListener('click', (event) => {
                if (event.target === overlay) close(false);
            });
            cancelButton.onclick = () => close(false);
            confirmButton.onclick = () => close(true);
            document.addEventListener('keydown', onKeydown, true);

            (options.danger ? cancelButton : confirmButton).focus();
        });
    }

    window.DiscRevealModal = Object.freeze({ confirm });
})();
