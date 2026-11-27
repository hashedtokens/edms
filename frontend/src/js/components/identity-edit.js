(function () {
  'use strict';

  // ============================================================
  // SHARED "EDIT IDENTITY" MODAL MARKUP
  // Consolidates Name + Tags + Annotation into a single popup
  // used across Repoview, Webview and Collection views.
  // ============================================================

  function escapeHtml(value) {
    const div = document.createElement('div');
    div.textContent = String(value ?? '');
    return div.innerHTML;
  }

  function escapeAttr(value) {
    return escapeHtml(value).replace(/"/g, '&quot;');
  }

  function renderIdentityEditModal({ title, name, tags, annotation }) {
    const tagChips = (Array.isArray(tags) ? tags : [])
      .map(
        (tag) => `
            <span class="inline-flex items-center gap-1.5 rounded-md
             border border-cyan-700/40 dark:border-cyan-500/30
             bg-cyan-200 dark:bg-cyan-500/10
             px-2 py-0.5 text-xs
             text-cyan-900 dark:text-cyan-300">
                <span>${escapeHtml(tag)}</span>
                <button type="button" data-remove-tag class="ml-0.5 rounded px-1 leading-none
             text-cyan-800 dark:text-cyan-400
             hover:bg-cyan-300 dark:hover:bg-cyan-500/30
             hover:text-cyan-950 dark:hover:text-white transition cursor-pointer"
             title="Remove ${escapeAttr(tag)}">&times;</button>
            </span>
        `
      )
      .join('');

    return `
        <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">${escapeHtml(title)}</h2>

        <label class="mb-1 block text-[11px] text-slate-500">Name <span class="text-rose-500 dark:text-rose-400">*</span></label>
        <input id="editIdentityName" value="${escapeAttr(name)}" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">

        <label class="mb-1 block text-[11px] text-slate-500">Tags</label>
        <div id="editIdentityTagsList" class="flex flex-wrap gap-1.5 mb-2 empty:hidden">${tagChips}</div>
        <div class="flex items-center gap-2 mb-4">
            <input id="editIdentityTags" class="h-9 flex-1 rounded-md
                   border border-slate-300 dark:border-slate-700
                   bg-white dark:bg-slate-950
                   px-3 text-xs
                   text-slate-800 dark:text-slate-200
                   placeholder:text-slate-400 dark:placeholder:text-slate-600
                   outline-none focus:border-cyan-500"
            placeholder="Type a tag and press Enter">
            <button id="editIdentityAddTagBtn" type="button" class="h-9 rounded-md border
                   border-slate-300 dark:border-slate-700
                   bg-slate-100 dark:bg-slate-800
                   px-3 text-xs
                   text-slate-700 dark:text-slate-300
                   hover:bg-slate-200 dark:hover:bg-slate-700
                   hover:text-slate-900 dark:hover:text-white
                   transition">
                Add
            </button>
        </div>

        <label class="mb-1 block text-[11px] text-slate-500">Annotation</label>
        <textarea id="editIdentityAnnotation" class="mb-4 h-32 w-full resize-none rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 py-2 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200 placeholder:text-slate-400 dark:placeholder:text-slate-600" placeholder="Notes...">${escapeHtml(annotation || '')}</textarea>

        <div class="mt-4 flex justify-end gap-2">
            <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
            <button id="editIdentitySave" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Save</button>
        </div>
    `;
  }

  window.renderIdentityEditModal = renderIdentityEditModal;
})();
