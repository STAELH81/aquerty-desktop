(function () {
  var cfg = window.AQUERTY_CONFIG || {};

  function bind(sel, url) {
    if (!url) return;
    document.querySelectorAll(sel).forEach(function (el) {
      el.setAttribute("href", url);
      if (sel.indexOf("download") === -1) {
        el.setAttribute("target", "_blank");
        el.setAttribute("rel", "noopener");
      }
    });
  }

  bind("[data-download]", cfg.downloadUrl);
  bind("[data-buy-life]", cfg.gumroadLifetime);
  bind("[data-buy-year]", cfg.gumroadAnnual);
})();
