import { Editor } from 'https://esm.sh/@tiptap/core@2.11.5';
import StarterKit from 'https://esm.sh/@tiptap/starter-kit@2.11.5';
import Link from 'https://esm.sh/@tiptap/extension-link@2.11.5';
import Placeholder from 'https://esm.sh/@tiptap/extension-placeholder@2.11.5';

const EMPTY_DOC = { type: 'doc', content: [{ type: 'paragraph' }] };
let frontPageEditor = null;
let activeWebId = null;
let isSaving = false;

const escapeHtml = (value = '') => String(value).replace(/[&<>"']/g, char => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
}[char]));

function getFrontPageEndpoint(name) {
  const template = window.WEBVIEW_FRONT_PAGE_API_URL || '/api/webview/:name/front-page';
  return template
    .replace(':name', encodeURIComponent(String(name)))
    .replace(':id', encodeURIComponent(String(name)));
}

async function loadFrontPage(web) {
  // Prefer a configured backend. If this WebView object already contains the
  // document, it can be opened without a separate GET request.
  if (web.front_page && typeof web.front_page === 'object') {
    return web.front_page;
  }
  if (web.frontpageContent && typeof web.frontpageContent === 'object') {
    return web.frontpageContent;
  }
  if (web.frontPageContent && typeof web.frontPageContent === 'object') {
    return web.frontPageContent;
  }
  if (web.content && typeof web.content === 'object') {
    return web.content;
  }

  try {
    const webName = web.name || web.id;
    const response = await fetch(getFrontPageEndpoint(webName), {
      method: 'GET',
      headers: { Accept: 'application/json' }
    });
    if (!response.ok) {
      console.warn(`Backend returned ${response.status}. Using hardcoded default data.`);
      return EMPTY_DOC;
    }
    const payload = await response.json();
    return payload.front_page || payload.content || payload.frontpageContent || payload.frontPageContent || EMPTY_DOC;
  } catch (error) {
    console.warn("Could not load from backend, using hardcoded default data.", error);
    return EMPTY_DOC;
  }
}

/** Render the editor JSON document as a full styled HTML page string for preview.
 *  Mirrors the styling used by webview-preview.js `buildPage` (Tailwind CDN +
 *  inline CSS) so the saved preview looks consistent with the client-side preview. */
function renderFullPage(doc, web) {
  const bodyHtml = typeof window.renderTiptapJsonToHtml === 'function'
    ? window.renderTiptapJsonToHtml(doc || {})
    : '';
  const title = (web && web.name) ? escapeHtml(String(web.name)) : 'WebView';
  const annotation = (web && web.annotation) ? escapeHtml(String(web.annotation)) : '';
  const annotationHtml = annotation
    ? `<span class="annotation">${annotation}</span>`
    : '';
  const safeBody = bodyHtml.trim()
    ? bodyHtml
    : '<p>No content has been added to this WebView yet.</p>';
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${title}</title>
  <script src="https://cdn.tailwindcss.com"><\/script>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Source+Code+Pro:wght@200..900&display=swap" rel="stylesheet">
  <style>
    * { box-sizing: border-box; }
    body { margin: 0; background: #f9fafb; color: #1f2937; font-family: Arial, Helvetica, sans-serif; }
    .navbar { border-bottom: 2px solid rgba(0,0,0,.25); background: white; }
    .nav-inner { max-width: 1200px; margin: auto; padding: 10px 18px; display:flex; align-items:center; justify-content:space-between; gap:24px; }
    .brand { display:flex; align-items:center; gap:12px; color:#2d5016; font-size:24px; font-weight:800; white-space:nowrap; }
    .brand-mark { width:38px; height:38px; border-radius:9px; display:grid; place-items:center; color:white; background:#2d5016; font-size:17px; }
    .nav-links { display:flex; align-items:center; flex-wrap:wrap; gap:4px; }
    .nav-links a { color:rgba(0,0,0,.8); text-decoration:none; padding:10px 14px; border-bottom:2px solid transparent; font-weight:600; }
    .nav-links a:hover { border-bottom-color:rgba(0,0,0,.4); }
    .page { max-width:960px; margin:30px auto 90px; background:#fff; border:1px solid #e5e7eb; border-radius:12px; overflow:hidden; box-shadow:0 12px 30px rgba(0,0,0,.08); }
    .page-header { padding:42px 48px; border-bottom:4px solid #4a154b; }
    .page-title { margin:0; color:#4a154b; font-size:clamp(2rem,5vw,3.2rem); font-weight:800; letter-spacing:-.03em; overflow-wrap:anywhere; }
    .annotation { display:inline-flex; margin-top:18px; padding:6px 15px; border:1px solid #d5c5d6; border-radius:999px; background:#eadeeb; color:#4a154b; font-size:14px; font-weight:700; }
    .document-content { padding:34px 48px 48px; line-height:1.75; overflow-wrap:anywhere; }
    .document-content h1,.document-content h2,.document-content h3,.document-content h4 { color:#4a154b; line-height:1.25; font-weight:750; margin:1.6em 0 .65em; }
    .document-content h1 { font-size:2.2rem; margin-top:.1em; }
    .document-content h2 { font-size:1.7rem; }
    .document-content h3 { font-size:1.35rem; }
    .document-content p { margin:.9em 0; }
    .document-content ul,.document-content ol { margin:1em 0; padding-left:1.8rem; }
    .document-content ul { list-style:disc; }
    .document-content ol { list-style:decimal; }
    .document-content li { padding-left:.2rem; margin:.3rem 0; }
    .document-content blockquote { margin:1.4em 0; padding:.7em 1.2em; border-left:4px solid #4a154b; background:#f8f5f8; color:#514451; }
    .document-content pre { padding:18px; overflow:auto; border-radius:8px; background:#111827; color:#f9fafb; font-family:'Source Code Pro',monospace; line-height:1.55; }
    .document-content code { padding:.12em .35em; border-radius:4px; background:#f3f4f6; font-family:'Source Code Pro',monospace; font-size:.92em; }
    .document-content pre code { padding:0; background:transparent; color:inherit; }
    .document-content a { color:#6d2870; text-decoration:underline; text-underline-offset:3px; }
    .document-content img { max-width:100%; height:auto; border-radius:8px; }
    .document-content hr { margin:2rem 0; border:0; border-top:1px solid #d1d5db; }
    .footer { position:fixed; bottom:0; left:0; width:100%; min-height:46px; display:flex; align-items:center; justify-content:center; padding:10px; background:#4a154b; color:#eadeeb; font-weight:600; box-shadow:0 -2px 8px rgba(0,0,0,.08); }
    @media(max-width:650px) { .nav-inner { align-items:flex-start; flex-direction:column; gap:8px; } .nav-links a { padding:8px; } .page { margin:14px 10px 80px; } .page-header { padding:28px 22px; } .document-content { padding:24px 22px 32px; } }
  </style>
</head>
<body>
  <nav class="navbar"><div class="nav-inner">
    <div class="brand"><div class="brand-mark" aria-hidden="true">HT</div><span>HashedTokens</span></div>
    <div class="nav-links">
      <a href="/dashboard.html">Home</a><a href="/about-collection.html" aria-current="page">About collection</a><a href="/webview.html">Cards</a><a href="/listview.html">List</a>
    </div>
  </div></nav>
  <main class="page">
    <header class="page-header"><h1 class="page-title">${title}</h1>${annotationHtml}</header>
    <article class="document-content">${safeBody}</article>
  </main>
  <footer class="footer">Powered by Asvatthi</footer>
</body>
</html>`;
}

async function persistFrontPage(web, content) {
  try {
    const webName = web.name || web.id;
    // Generate the full preview HTML from the same document being saved so the
    // backend preview (`/webview/:name/preview`) never goes stale. The `html`
    // field is optional — only send it when non-blank (a blank html is a 400).
    const fullHtml = renderFullPage(content, web);
    const payload = { front_page: content };
    if (fullHtml.trim()) {
      payload.html = fullHtml;
    }
    const response = await fetch(getFrontPageEndpoint(webName), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
      body: JSON.stringify(payload)
    });
    if (!response.ok) {
      console.warn(`Could not save front-page content to backend (${response.status}). Using local state only.`);
      throw new Error("Backend save failed", response.error);
    } else {
      const result = await response.json().catch(() => ({}));
      content = result.front_page || result.content;
    }
  } catch (error) {
    console.warn("Backend save failed, using local state only.", error);
    throw new Error("Backend save failed", error);
  }

  web.front_page = content;
  web.frontpageContent = content;
  web.content = content;
  return { front_page: content };
}

function editorMarkup(web) {
  return `
    <section class="edit-frontpage-modal" aria-label="Edit WebView front page">
      <header class="flex items-center justify-between gap-3 mb-4">
        <div>
          <h2 class="text-sm font-semibold text-slate-800 dark:text-white">Edit WebView Front Page</h2>
          <p class="mt-1 text-[11px] text-slate-500">${escapeHtml(web.name || web.title || 'WebView')}</p>
        </div>
        <span id="frontPageSaveStatus" class="text-[11px] text-slate-500" role="status">Loading content…</span>
      </header>

      <div class="frontpage-rte-toolbar flex flex-wrap gap-1 mb-3" role="toolbar" aria-label="Text formatting">
        <button type="button" data-frontpage-command="bold" class="frontpage-tool" title="Bold"><strong>B</strong></button>
        <button type="button" data-frontpage-command="italic" class="frontpage-tool" title="Italic"><em>I</em></button>
        <button type="button" data-frontpage-command="strike" class="frontpage-tool" title="Strikethrough"><s>S</s></button>
        <span class="frontpage-toolbar-divider"></span>
        <button type="button" data-frontpage-command="h1" class="frontpage-tool" title="Heading 1">H1</button>
        <button type="button" data-frontpage-command="h2" class="frontpage-tool" title="Heading 2">H2</button>
        <button type="button" data-frontpage-command="paragraph" class="frontpage-tool" title="Paragraph">¶</button>
        <span class="frontpage-toolbar-divider"></span>
        <button type="button" data-frontpage-command="bulletList" class="frontpage-tool" title="Bulleted list">• List</button>
        <button type="button" data-frontpage-command="orderedList" class="frontpage-tool" title="Numbered list">1. List</button>
        <button type="button" data-frontpage-command="blockquote" class="frontpage-tool" title="Blockquote">❝</button>
        <button type="button" data-frontpage-command="codeBlock" class="frontpage-tool" title="Code block">&lt;/&gt;</button>
        <span class="frontpage-toolbar-divider"></span>
        <button type="button" data-frontpage-command="undo" class="frontpage-tool" title="Undo">↶</button>
        <button type="button" data-frontpage-command="redo" class="frontpage-tool" title="Redo">↷</button>
      </div>

      <div id="frontPageRte" class="frontpage-rte-surface" aria-label="Front-page rich text editor"></div>

      <footer class="flex items-center justify-end gap-2 mt-4">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
        <button id="saveFrontPageContent" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Save content</button>
      </footer>
    </section>
  `;
}

function setStatus(message, isError = false) {
  const status = document.getElementById('frontPageSaveStatus');
  if (!status) return;
  status.textContent = message;
  status.classList.toggle('text-red-600', isError);
  status.classList.toggle('dark:text-red-400', isError);
  status.classList.toggle('text-slate-500', !isError);
}

function bindToolbar() {
  document.querySelectorAll('[data-frontpage-command]').forEach(button => {
    // Prevent the toolbar click from stealing selection from the editor.
    button.addEventListener('mousedown', event => event.preventDefault());
    button.addEventListener('click', () => {
      if (!frontPageEditor) return;
      const chain = frontPageEditor.chain().focus();
      const commands = {
        bold: () => chain.toggleBold(),
        italic: () => chain.toggleItalic(),
        strike: () => chain.toggleStrike(),
        h1: () => chain.toggleHeading({ level: 1 }),
        h2: () => chain.toggleHeading({ level: 2 }),
        paragraph: () => chain.setParagraph(),
        bulletList: () => chain.toggleBulletList(),
        orderedList: () => chain.toggleOrderedList(),
        blockquote: () => chain.toggleBlockquote(),
        codeBlock: () => chain.toggleCodeBlock(),
        undo: () => chain.undo(),
        redo: () => chain.redo()
      };
      commands[button.dataset.frontpageCommand]?.().run();
    });
  });
}

function updateToolbarState() {
  if (!frontPageEditor) return;
  document.querySelectorAll('[data-frontpage-command]').forEach(button => {
    const command = button.dataset.frontpageCommand;
    let isActive = false;

    if (command === 'bold') isActive = frontPageEditor.isActive('bold');
    else if (command === 'italic') isActive = frontPageEditor.isActive('italic');
    else if (command === 'strike') isActive = frontPageEditor.isActive('strike');
    else if (command === 'h1') isActive = frontPageEditor.isActive('heading', { level: 1 });
    else if (command === 'h2') isActive = frontPageEditor.isActive('heading', { level: 2 });
    else if (command === 'paragraph') isActive = frontPageEditor.isActive('paragraph');
    else if (command === 'bulletList') isActive = frontPageEditor.isActive('bulletList');
    else if (command === 'orderedList') isActive = frontPageEditor.isActive('orderedList');
    else if (command === 'blockquote') isActive = frontPageEditor.isActive('blockquote');
    else if (command === 'codeBlock') isActive = frontPageEditor.isActive('codeBlock');

    button.classList.toggle('is-active', isActive);
  });
}

function teardownFrontPageEditor() {
  frontPageEditor?.destroy();
  frontPageEditor = null;
  activeWebId = null;
  isSaving = false;
}

/** Open the RTE popup for the selected WebView's front-page content. */
export async function openEditWebFrontPageModal(id) {
  // These functions are supplied by the existing List View application.
  const web = typeof window.getWeb === 'function' ? window.getWeb(id) : null;
  if (!web) {
    (window.showAlert || window.alert)('Could not find this WebView.');
    return;
  }

  teardownFrontPageEditor();
  activeWebId = id;
  window.openModal(editorMarkup(web));
  bindToolbar();

  const saveButton = document.getElementById('saveFrontPageContent');
  saveButton?.addEventListener('click', async () => {
    if (!frontPageEditor || isSaving || String(activeWebId) !== String(id)) return;
    isSaving = true;
    saveButton.disabled = true;
    saveButton.textContent = 'Saving…';
    try {
      const content = frontPageEditor.getJSON();
      await persistFrontPage(web, content);
      setStatus('Saved successfully');
      if (typeof window.showAlert === 'function') window.showAlert('WebView front-page content saved.');
      if (typeof window.closeModal === 'function') window.closeModal();
      teardownFrontPageEditor();
    } catch (error) {
      console.log(error);
      setStatus(error.message || 'Save failed.', true);
      (window.showAlert || window.alert)(error.message || 'Could not save front-page content.');
    } finally {
      isSaving = false;
      if (saveButton.isConnected) {
        saveButton.disabled = false;
        saveButton.textContent = 'Save content';
      }
    }
  });

  try {
    const content = await loadFrontPage(web);
    // The user may have closed the modal while the request was in flight.
    const editorElement = document.getElementById('frontPageRte');
    if (!editorElement || String(activeWebId) !== String(id)) return;
    frontPageEditor = new Editor({
      element: editorElement,
      extensions: [
        StarterKit,
        Link.configure({ openOnClick: false, autolink: true }),
        Placeholder.configure({ placeholder: 'Write the WebView front-page content…' })
      ],
      content: content || EMPTY_DOC,
      editorProps: {
        attributes: {
          class: 'frontpage-prosemirror',
          'aria-label': 'Editable WebView front-page content',
          spellcheck: 'true'
        }
      },
      onUpdate: () => setStatus('Unsaved changes'),
      onTransaction: () => updateToolbarState()
    });
    setStatus('Ready to edit');
  } catch (error) {
    setStatus(error.message || 'Could not load content.', true);
    (window.showAlert || window.alert)(error.message || 'Could not load front-page content.');
  }
}

// Optional cleanup hook for the shared modal close handler.
export function closeEditWebFrontPageModal() {
  teardownFrontPageEditor();
}

/** Preview WebView page. Delegates to global previewWebView or previewWebViewPage. */
export function previewWebView(id, options = {}) {
  if (typeof window.previewWebView === 'function' && window.previewWebView !== previewWebView) {
    return window.previewWebView(id, options);
  }
  if (typeof window.previewWebViewPage === 'function') {
    return window.previewWebViewPage(id, options);
  }
  const web = typeof window.getWeb === 'function' ? window.getWeb(id) : null;
  return web;
}

// Expose a global for existing non-module scripts. Ensure this file is loaded
// with <script type="module"> for the Tiptap imports to work.
window.openEditWebFrontPageModal = openEditWebFrontPageModal;
window.closeEditWebFrontPageModal = closeEditWebFrontPageModal;
window.previewWebView = window.previewWebView || previewWebView;