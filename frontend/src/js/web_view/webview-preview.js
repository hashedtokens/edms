/*
 * Vanilla JS WebView preview renderer. No React dependency.
 * Include this file after your existing app scripts.
 * Usage: function previewWebView(id) { return previewWebViewPage(id); }
 */
(function (global) {
  "use strict";

  const MOCK_WEBVIEW = {
    id: "mock-content-error",
    name: "Content Unavailable — Error Preview",
    annotation: "MOCK DATA · Backend not connected",
    isMock: true,
    content: {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Content Unavailable" }],
        },
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "We couldn't retrieve the latest content for this page. The content may not have been updated successfully, or the server may have encountered an issue while fetching it.",
            },
          ],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "What you can do" }],
        },
        {
          type: "bulletList",
          content: [
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [
                    {
                      type: "text",
                      text: "Retry loading the page to fetch the latest saved content.",
                    },
                  ],
                },
              ],
            },
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [
                    {
                      type: "text",
                      text: "If you were editing this page, reopen the editor and try saving your changes again.",
                    },
                  ],
                },
              ],
            },
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [
                    {
                      type: "text",
                      text: "Check your network connection and ensure the server is accessible.",
                    },
                  ],
                },
              ],
            },
          ],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Still experiencing this issue?" }],
        },
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "If the problem persists after retrying, please contact the ",
            },
            {
              type: "text",
              marks: [{ type: "bold" }],
              text: "HashedToken support team",
            },
            {
              type: "text",
              text: " and provide the page details along with any error messages you encountered. Our team will help investigate the issue.",
            },
          ],
        },
        {
          type: "blockquote",
          content: [
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  text: "Your changes may not have been saved successfully. Before leaving the editor, verify that your latest changes have been saved to avoid losing your work.",
                },
              ],
            },
          ],
        },
        {
          type: "horizontalRule",
        },
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "We apologize for the inconvenience and appreciate your patience while we resolve this issue.",
            },
          ],
        },
      ],
    },
  };

  const escapeHtml = (value) =>
    String(value ?? "").replace(
      /[&<>"']/g,
      (char) =>
        ({
          "&": "&amp;",
          "<": "&lt;",
          ">": "&gt;",
          '"': "&quot;",
          "'": "&#39;",
        })[char],
    );

  function renderMarks(text, marks = []) {
    let output = escapeHtml(text).replace(/\n/g, "<br>");
    for (const mark of marks) {
      switch (mark.type) {
        case "bold":
          output = `<strong>${output}</strong>`;
          break;
        case "italic":
          output = `<em>${output}</em>`;
          break;
        case "strike":
          output = `<s>${output}</s>`;
          break;
        case "code":
          output = `<code>${output}</code>`;
          break;
        case "underline":
          output = `<u>${output}</u>`;
          break;
        case "link": {
          const href = String(mark.attrs?.href || "#");
          const safeHref = /^(https?:|mailto:|\/|#)/i.test(href)
            ? escapeHtml(href)
            : "#";
          output = `<a href="${safeHref}" target="_blank" rel="noopener noreferrer">${output}</a>`;
          break;
        }
      }
    }
    return output;
  }

  function renderNode(node) {
    if (!node || typeof node !== "object") return "";
    const children = Array.isArray(node.content)
      ? node.content.map(renderNode).join("")
      : "";
    switch (node.type) {
      case "doc":
        return children;
      case "text":
        return renderMarks(node.text || "", node.marks || []);
      case "paragraph":
        return `<p>${children || "<br>"}</p>`;
      case "heading": {
        const level = Math.min(6, Math.max(1, Number(node.attrs?.level) || 1));
        return `<h${level}>${children}</h${level}>`;
      }
      case "bulletList":
        return `<ul>${children}</ul>`;
      case "orderedList": {
        const start = Math.max(1, Number(node.attrs?.start) || 1);
        return `<ol start="${start}">${children}</ol>`;
      }
      case "listItem":
        return `<li>${children}</li>`;
      case "blockquote":
        return `<blockquote>${children}</blockquote>`;
      case "codeBlock":
        return `<pre><code>${(node.content || []).map((child) => escapeHtml(child.text || "")).join("")}</code></pre>`;
      case "horizontalRule":
        return "<hr>";
      case "hardBreak":
        return "<br>";
      case "image": {
        const src = String(node.attrs?.src || "");
        if (!/^(https?:|data:image\/)/i.test(src)) return "";
        return `<img src="${escapeHtml(src)}" alt="${escapeHtml(node.attrs?.alt || "")}">`;
      }
      default:
        return children;
    }
  }

  function buildPage(web, contentHtml) {
    const title = web.name || web.title || "Untitled Document";
    const annotation = web.annotation || "";
    const badge = web.isMock
      ? '<div class="mock-banner"><strong>DEMO ONLY</strong> — MOCK DATA is being shown because the backend/API did not return a WebView document.</div>'
      : "";

    return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(title)}</title>
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
    .mock-banner { margin:20px 28px 0; padding:12px 15px; border:1px solid #f59e0b; border-radius:8px; background:#fffbeb; color:#92400e; font-size:13px; line-height:1.5; }
    .mock-banner strong { letter-spacing:.06em; }
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
    @media(max-width:650px) { .nav-inner { align-items:flex-start; flex-direction:column; gap:8px; } .nav-links a { padding:8px; } .page { margin:14px 10px 80px; } .page-header { padding:28px 22px; } .document-content { padding:24px 22px 32px; } .mock-banner { margin:16px 16px 0; } }
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
    <header class="page-header"><h1 class="page-title">${escapeHtml(title)}</h1>${annotation ? `<span class="annotation">${escapeHtml(annotation)}</span>` : ""}</header>
    ${badge}
    <article class="document-content">${contentHtml || "<p>No content has been added to this WebView yet.</p>"}</article>
  </main>
  <footer class="footer">Powered by Asvatthi</footer>
</body>
</html>`;
  }

  async function fetchWebViewFromBackend(id, options = {}) {
    const name = options.name || id;
    const endpoint =
      options.endpoint ||
      (window.WEBVIEW_FRONT_PAGE_API_URL
        ? window.WEBVIEW_FRONT_PAGE_API_URL.replace(
            ":name",
            encodeURIComponent(String(name)),
          ).replace(":id", encodeURIComponent(String(name)))
        : `/api/webview/${encodeURIComponent(String(name))}/front-page`);
    const response = await fetch(endpoint, {
      method: "GET",
      headers: { Accept: "application/json" },
      signal: options.signal,
    });
    if (!response.ok)
      throw new Error(`WebView API returned HTTP ${response.status}`);
    const data = await response.json();
    // Accept either a direct WebView object or { data: WebView } API responses.
    const web = data && data.data ? data.data : data;
    const frontPage = web
      ? web.front_page ||
        web.content ||
        web.frontpageContent ||
        web.frontPageContent
      : null;
    if (!web || !frontPage || typeof frontPage !== "object") {
      throw new Error(
        "API response did not contain a Tiptap JSON 'front_page' document",
      );
    }
    web.front_page = frontPage;
    web.content = frontPage;
    return web;
  }

  async function previewWebViewPage(id, options = {}) {
    let web;
    let usingMock = false;

    try {
      if (typeof options.loadWebView === "function") {
        web = await options.loadWebView(id);
      } else {
        web = await fetchWebViewFromBackend(id, options);
      }
      const frontPage = web
        ? web.front_page ||
          web.content ||
          web.frontpageContent ||
          web.frontPageContent
        : null;
      if (!web || !frontPage || typeof frontPage !== "object") {
        throw new Error("No WebView front_page returned");
      }
      web.front_page = frontPage;
      web.content = frontPage;
    } catch (error) {
      console.warn(
        "WebView preview is using mock data because the API is unavailable:",
        error,
      );
      web = { ...MOCK_WEBVIEW };
      usingMock = true;
    }

    const html = renderNode(web.front_page || web.content);
    const page = buildPage({ ...web, isMock: usingMock || web.isMock }, html);
    const previewWindow = window.open("", "_blank");

    if (!previewWindow) {
      alert(
        "The preview was blocked by your browser. Allow pop-ups for this site and try again.",
      );
      return { webview: web, html: page, isMock: usingMock, opened: false };
    }

    previewWindow.document.open();
    previewWindow.document.write(page);
    previewWindow.document.close();
    return { webview: web, html: page, isMock: usingMock, opened: true };
  }

  // Optional helper if you want to preview local state first and only try the API
  // when local state has no content. Do not pass untrusted HTML through this path.
  global.previewWebViewPage = previewWebViewPage;
  global.renderTiptapJsonToHtml = renderNode;
  global.WEBVIEW_PREVIEW_MOCK_DATA = MOCK_WEBVIEW;
  if (!global.previewWebView) {
    global.previewWebView = function (id, options) {
      return previewWebViewPage(id, options);
    };
  }
})(window);
