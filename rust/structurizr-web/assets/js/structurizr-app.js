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
        var dark = isDarkMode();
        document.documentElement.setAttribute('data-bs-theme', dark ? 'dark' : 'light');
        var item = document.getElementById('themeToggle');
        if (item) item.setAttribute('aria-checked', String(dark));
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
        // The dark-mode entry in the "More" menu.
        var toggle = document.getElementById('themeToggle');
        if (toggle) {
            apply();
            toggle.addEventListener('click', function () {
                setDarkMode(!isDarkMode());
            });
        }

        // Menus (<details class="sx-more">) close on a click elsewhere, on
        // Escape, and once an action in them has run — but not when a radio
        // or checkbox in them is ticked, so settings can be changed together.
        document.querySelectorAll('.sx-more').forEach(function (menu) {
            menu.querySelectorAll('.sx-more-menu a, .sx-more-menu button').forEach(function (action) {
                action.addEventListener('click', function () { menu.open = false; });
            });
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
