// Varium — le site installable : service worker, bouton « Installer », partage.
// Aucun script en ligne : ce fichier est servi par le site (CSP script-src 'self').
(function () {
  "use strict";
  var script = document.currentScript;
  // La base du site (/VARIUM/ sur GitHub Pages) : le dossier de ce script.
  var base = script ? new URL(".", script.src) : new URL("/", location.href);

  // Service worker : pages, styles et icônes disponibles hors ligne.
  if ("serviceWorker" in navigator) {
    addEventListener("load", function () {
      navigator.serviceWorker
        .register(new URL("sw.js", base).href, { scope: base.pathname })
        .catch(function () {});
    });
  }

  function pret(f) {
    if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", f);
    else f();
  }

  // « Installer le site » : proposé quand le navigateur le permet.
  var invite = null;
  addEventListener("beforeinstallprompt", function (e) {
    e.preventDefault();
    invite = e;
    document.querySelectorAll(".pwa-installer").forEach(function (b) { b.hidden = false; });
  });
  addEventListener("appinstalled", function () {
    invite = null;
    document.querySelectorAll(".pwa-installer").forEach(function (b) { b.hidden = true; });
  });

  pret(function () {
    document.querySelectorAll(".partage").forEach(function (bloc) {
      var url = bloc.getAttribute("data-url") || location.href;
      var texte = bloc.getAttribute("data-texte") || document.title;
      var etat = bloc.querySelector(".partage-etat");
      var natif = bloc.querySelector(".partage-natif");
      var copier = bloc.querySelector(".partage-copier");
      var installer = bloc.querySelector(".pwa-installer");

      if (natif && navigator.share) {
        natif.hidden = false;
        natif.addEventListener("click", function () {
          navigator.share({ title: document.title, text: texte, url: url }).catch(function () {});
        });
      }
      if (copier && navigator.clipboard) {
        copier.hidden = false;
        copier.addEventListener("click", function () {
          navigator.clipboard.writeText(url).then(function () {
            if (etat) {
              etat.textContent = bloc.getAttribute("data-copie") || "OK";
              setTimeout(function () { etat.textContent = ""; }, 2500);
            }
          });
        });
      }
      if (installer) {
        if (invite) installer.hidden = false;
        installer.addEventListener("click", function () {
          if (!invite) return;
          invite.prompt();
          invite.userChoice.finally(function () {
            invite = null;
            installer.hidden = true;
          });
        });
      }
    });
  });
})();
