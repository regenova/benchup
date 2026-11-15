// Trunk supplies the final WASM file size, and counts decoded response bytes.
// Both quantities use the same units even with gzip/Brotli or a cached response;
// Content-Length would describe compressed bytes and give incorrect percentages.
export default function initializer({document = globalThis.document,
    messages = globalThis.window?.benchupStartupMessages} = {}) {
    let catalog;
    let phase = 'download';
    let downloaded = 0;
    let size = 0;
    const number = new Intl.NumberFormat(document.documentElement.lang || 'en', {
        minimumFractionDigits: 1, maximumFractionDigits: 1,
    });
    const tr = (key, args = {}) => catalog?.[key]?.replace(/\{(\w+)\}/g,
        (_, name) => args[name]);

    const render = () => {
        // Rust removes these nodes once the GPU is ready, or replaces them
        // with its own renderer error. Late callbacks must leave that UI alone.
        const bar = document.getElementById('loading-progress');
        const label = document.getElementById('loading-download');
        if (!bar || !label) return;
        const knownSize = Number.isFinite(size) && size > 0;
        const percent = phase === 'starting' ? 100
            : knownSize ? Math.floor(Math.min(downloaded / size, 1) * 100) : null;
        bar.hidden = phase === 'failed';
        if (percent === null || phase === 'failed') {
            bar.removeAttribute('data-determinate');
            bar.removeAttribute('aria-valuenow');
        } else {
            bar.setAttribute('data-determinate', '');
            bar.setAttribute('aria-valuenow', String(percent));
            bar.style.setProperty('--progress', `${percent}%`);
        }
        let text;
        if (phase === 'failed') text = tr('web.download_failed');
        else if (phase === 'starting') text = tr('web.starting');
        else if (knownSize) {
            text = tr('web.download_progress', {percent,
                current: number.format(downloaded / 1_000_000),
                total: number.format(size / 1_000_000)}) ?? `${percent}%`;
        } else text = tr('web.downloading');
        label.textContent = text ?? '';
    };
    // The catalog and WASM download run independently. Replay the current state
    // when translations arrive instead of resetting an in-progress download.
    Promise.resolve(messages).then(value => { catalog = value; render(); }).catch(() => {});
    return {
        onStart() { render(); },
        onProgress({current, total}) {
            downloaded = Number.isFinite(current) ? Math.max(0, current) : 0;
            size = total;
            if (Number.isFinite(size) && size > 0 && downloaded >= size) phase = 'starting';
            render();
        },
        // Download completion is measurable; compilation/GPU setup is not.
        // Keep the overlay until WebRunner::start removes it in main.rs.
        onSuccess() { phase = 'starting'; render(); },
        onFailure() { phase = 'failed'; render(); },
    };
}
