// Checks whether the user arrived at a problem-code page from an older
// release and, if so, displays a non-intrusive banner suggesting an update.
//
// The current release version is read from a <meta name="ironplc-version">
// tag injected by the Sphinx build.  The user's version comes from the
// ?version= query parameter appended to problem-code URLs by the compiler
// and editor.

(function () {
  'use strict';

  var meta = document.querySelector('meta[name="ironplc-version"]');
  if (!meta) {
    return;
  }
  var currentVersion = meta.getAttribute('content');
  if (!currentVersion) {
    return;
  }

  var params = new URLSearchParams(window.location.search);
  var userVersion = params.get('version');
  if (!userVersion) {
    return;
  }

  if (!isOlderVersion(userVersion, currentVersion)) {
    return;
  }

  var box = document.createElement('div');
  box.className = 'ironplc-version-notice';
  box.innerHTML =
    '<strong>Newer version available:</strong> You are running IronPLC ' +
    escapeHtml(userVersion) +
    '. The latest release is ' +
    escapeHtml(currentVersion) +
    '. This issue may already be resolved. See ' +
    '<a href="/how-to-guides/update.html">update instructions</a>.';

  // Insert at the top of the page content area.
  var content = document.querySelector('.body, [role="main"], article');
  if (content) {
    content.insertBefore(box, content.firstChild);
  }

  // Compare two version strings (e.g. "0.159.0" < "0.160.0").
  // Returns true when userVer is strictly older than currentVer.
  function isOlderVersion(userVer, currentVer) {
    var a = userVer.split('.').map(Number);
    var b = currentVer.split('.').map(Number);
    var len = Math.max(a.length, b.length);
    for (var i = 0; i < len; i++) {
      var ai = i < a.length ? a[i] : 0;
      var bi = i < b.length ? b[i] : 0;
      if (isNaN(ai) || isNaN(bi)) {
        return false;
      }
      if (ai < bi) {
        return true;
      }
      if (ai > bi) {
        return false;
      }
    }
    return false;
  }

  function escapeHtml(str) {
    var div = document.createElement('div');
    div.appendChild(document.createTextNode(str));
    return div.innerHTML;
  }
})();
