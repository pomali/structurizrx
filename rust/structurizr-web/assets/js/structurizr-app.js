/*
 * Behaviour of the app bar every page shares (src/chrome.rs): the theme
 * toggle and the "More" menu.
 *
 * The theme lives in the same cookie structurizr-ui.js reads
 * (structurizr.renderingMode: "light", "dark" or absent for the system
 * setting), so a choice made on any page holds on every other. This script
 * only sets Bootstrap's data-bs-theme; a page that draws diagrams listens for
 * the "structurizr:themechange" event on document to redraw them.
 */
(function () {
    'use strict';

    var COOKIE = 'structurizr.renderingMode';
    var media = window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null;

    function renderingMode() {
        var match = document.cookie.match(/(?:^|;\s*)structurizr\.renderingMode=(light|dark)/);
        return match ? match[1] : '';
    }

    function isDarkMode() {
        var mode = renderingMode();
        if (mode) return mode === 'dark';
        return !!(media && media.matches);
    }

    function apply() {
        document.documentElement.setAttribute('data-bs-theme', isDarkMode() ? 'dark' : 'light');
    }

    function announce() {
        document.dispatchEvent(new CustomEvent('structurizr:themechange', {
            detail: { dark: isDarkMode() }
        }));
    }

    function setDarkMode(dark) {
        var nextYear = new Date();
        nextYear.setFullYear(nextYear.getFullYear() + 1);
        document.cookie = COOKIE + '=' + (dark ? 'dark' : 'light') +
            '; expires=' + nextYear.toUTCString() + '; path=/';
        apply();
        announce();
    }

    // Before first paint where possible: this script is loaded in <body>, so
    // the page is not yet rendered when it runs.
    apply();

    if (media && media.addEventListener) {
        media.addEventListener('change', function () {
            if (!renderingMode()) {
                apply();
                announce();
            }
        });
    }

    function init() {
        var toggle = document.getElementById('themeToggle');
        if (toggle) {
            toggle.addEventListener('click', function () {
                setDarkMode(!isDarkMode());
                toggle.blur();
            });
        }

        // Close the "More" menu on a click elsewhere or Escape.
        document.querySelectorAll('.sx-more').forEach(function (menu) {
            document.addEventListener('click', function (event) {
                if (menu.open && !menu.contains(event.target)) menu.open = false;
            });
            document.addEventListener('keydown', function (event) {
                if (event.key === 'Escape' && menu.open) {
                    menu.open = false;
                    menu.querySelector('summary').focus();
                }
            });
        });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

    window.StructurizrApp = { isDarkMode: isDarkMode, setDarkMode: setDarkMode };
})();
