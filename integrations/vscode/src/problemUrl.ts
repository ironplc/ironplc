// Builds documentation URLs for problem codes.
//
// Kept free of any `vscode` import so it can be unit-tested directly (the
// functional tests exercise the editor integration; this pure helper does not
// need the editor host).

/**
 * The www.ironplc.com reference section a problem code documents into.
 *
 * `E####` are editor problems, `P####` compiler problems, and `V####` runtime
 * (VM) problems. Anything else falls back to `editor` (the extension only
 * raises E-codes today).
 */
function sectionForCode(code: string): string {
  switch (code.charAt(0)) {
    case 'P':
      return 'compiler';
    case 'V':
      return 'runtime';
    default:
      return 'editor';
  }
}

/**
 * Builds the documentation URL for a problem code, tagged so PostHog can
 * attribute the arrival to the editor extension.
 *
 * `channel=extension` identifies the channel (we do not assume the editor is VS
 * Code) and `version` stays for the out-of-date banner in
 * docs/_static/version-check.js; PostHog captures both as breakdown dimensions
 * via `custom_campaign_params` in docs/_static/posthog-init.js.
 */
export function problemHelpUrl(code: string, version: string): string {
  const v = encodeURIComponent(version);
  return 'https://www.ironplc.com/reference/' + sectionForCode(code)
    + '/problems/' + code + '.html'
    + '?version=' + v
    + '&channel=extension';
}
