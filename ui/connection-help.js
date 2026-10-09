// File: connection-help.js
// Description: Tabularis UI extension (slot connection-modal.extra_fields) that explains which Cloudflare values go into the generic Host / Username / Password fields of the connection form.
// Author: Jose-Jorge HERNANDEZ
// Company: Parlee Conseiller, Inc.
// Date: 2026-10-09
// Last edit date: 2026-10-09
// Version: 1.0.0
//
// Hand-written IIFE (no build step). The host runs this source inside
// `new Function("React", "ReactJSXRuntime", "__TABULARIS_API__", source)` and
// reads the `__tabularis_plugin__` variable, which may be the component itself.
// Colours are inherited from the host theme (currentColor + opacity) so the box
// works in every theme.

var __tabularis_plugin__ = (function () {
  // Driver id this banner applies to (must equal `id` in .tabularium).
  var DRIVER_ID = "cloudflare-d1";
  // Where users create the API token.
  var TOKEN_URL = "https://dash.cloudflare.com/profile/api-tokens";

  // One row of the mapping table: form field -> what to enter.
  var ROWS = [
    ["Username", "Your Cloudflare Account ID (Dashboard → Workers & Pages, right sidebar)."],
    ["Password", "A Cloudflare API token with the “D1 Edit” permission."],
    ["Host / Port", "Not used — leave them empty."],
  ];

  var h = React.createElement;

  // Shared inline styles; opacity-based so they follow the active theme.
  var boxStyle = {
    border: "1px solid currentColor",
    borderColor: "rgba(128,128,128,0.4)",
    borderRadius: "8px",
    padding: "10px 12px",
    fontSize: "12px",
    lineHeight: 1.5,
  };
  var titleStyle = { fontWeight: 600, marginBottom: "6px" };
  var labelStyle = { fontWeight: 600, whiteSpace: "nowrap", paddingRight: "12px", verticalAlign: "top" };
  var linkStyle = {
    background: "none",
    border: "none",
    padding: 0,
    marginTop: "6px",
    cursor: "pointer",
    color: "inherit",
    textDecoration: "underline",
    fontSize: "12px",
  };

  function ConnectionHelp(props) {
    var ctx = (props && props.context) || {};
    // Only show for this plugin's own connection form.
    if (ctx.driver !== DRIVER_ID) return null;

    var rows = ROWS.map(function (r) {
      return h(
        "tr",
        { key: r[0] },
        h("td", { style: labelStyle }, r[0]),
        h("td", { style: { opacity: 0.85 } }, r[1])
      );
    });

    // openUrl is part of the host plugin API; guard in case it is absent.
    var api = typeof __TABULARIS_API__ !== "undefined" ? __TABULARIS_API__ : null;
    var link =
      api && typeof api.openUrl === "function"
        ? h(
            "button",
            {
              type: "button",
              style: linkStyle,
              onClick: function () {
                api.openUrl(TOKEN_URL);
              },
            },
            "Create an API token →"
          )
        : null;

    return h(
      "div",
      { style: boxStyle },
      h("div", { style: titleStyle }, "Cloudflare D1 connection"),
      h("table", { style: { borderCollapse: "collapse" } }, h("tbody", null, rows)),
      link
    );
  }

  return ConnectionHelp;
})();
