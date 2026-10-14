//! The app bar every page shares: brand, workspace, the tabs between the
//! workspace's pages, a "More" menu and the theme toggle.
//!
//! It is rendered here rather than repeated in each template so the pages
//! cannot drift apart again. Pages substitute it for `{{APP_HEADER}}`; the
//! styles are `assets/css/structurizr-app.css` and the behaviour (theme,
//! menu) is `assets/js/structurizr-app.js`, which every page loads.

/// Which tab, if any, the page belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Overview,
    Diagrams,
    Graph,
    Clusters,
    Review,
    Compare,
    Decisions,
    /// A page reachable from the "More" menu (print, canvas).
    None,
}

/// The workspace a page is about.
pub struct WorkspaceHeader<'a> {
    /// URL slug (`/workspace/{slug}`).
    pub slug: &'a str,
    /// Human-readable name shown in the bar.
    pub title: &'a str,
    /// Whether to offer the Decisions tab.
    pub has_decisions: bool,
}

const TABS: &[(Tab, &str, &str, &str)] = &[
    (Tab::Overview, "/overview", "bi-house", "Overview"),
    (Tab::Diagrams, "", "bi-diagram-2", "Diagrams"),
    (Tab::Graph, "/graph", "bi-diagram-3", "Graph"),
    (Tab::Clusters, "/clusters", "bi-grid-3x3", "Clusters"),
    (Tab::Review, "/review", "bi-list-check", "Review"),
    (Tab::Compare, "/diff", "bi-clock-history", "Compare"),
    (Tab::Decisions, "/decisions", "bi-journal-text", "Decisions"),
];

const BRAND: &str =
    r#"<img src="/static/img/structurizr-logo.png" alt="Structurizr" height="22"/>"#;

/// Opens the find-anything palette (`structurizr-palette.js`), which fills in
/// the platform's shortcut.
const FIND_BUTTON: &str = r#"<button type="button" class="sx-find" title="Find a view, element, decision or command" aria-label="Find"><i class="bi bi-search"></i><span class="sx-find-text">Find…</span><kbd class="sx-find-key">⌘K</kbd></button>"#;

/// The last entry of every "More" menu; `structurizr-app.js` keeps
/// `aria-checked` in step with the theme.
const THEME_ITEM: &str = r#"<div class="sx-menu-sep"></div><button type="button" id="themeToggle" role="menuitemcheckbox" aria-checked="false"><i class="bi bi-moon"></i>Dark mode<i class="bi bi-check-lg sx-check"></i></button>"#;

/// The "More" (⋯) menu: `items` (menu entries as HTML) then the theme.
fn more_menu(items: &str) -> String {
    format!(
        r#"<details class="sx-more"><summary class="btn btn-sm btn-outline-secondary" title="More" aria-label="More"><i class="bi bi-three-dots"></i></summary><div class="sx-more-menu" role="menu">{items}{THEME_ITEM}</div></details>"#
    )
}

/// The app bar for a page served by `structurizrx serve`. `None` is the
/// workspace index, which has no workspace and so no tabs.
pub fn app_header(workspace: Option<&WorkspaceHeader>, active: Tab) -> String {
    // `data-workspace` is what the palette searches.
    let mut html = match workspace {
        Some(ws) => format!(
            r#"<header class="sx-appbar" data-workspace="{}">"#,
            escape(ws.slug)
        ),
        None => String::from(r#"<header class="sx-appbar">"#),
    };
    html.push_str(&format!(
        r#"<a class="sx-brand" href="/" title="All workspaces">{BRAND}</a>"#
    ));

    if let Some(ws) = workspace {
        let base = format!("/workspace/{}", escape(ws.slug));
        html.push_str(&format!(
            r#"<a class="sx-workspace" href="{base}/overview" title="{title}">{title}</a>"#,
            title = escape(ws.title)
        ));

        html.push_str(r#"<nav class="sx-tabs" aria-label="Workspace">"#);
        for &(tab, path, icon, label) in TABS {
            if tab == Tab::Decisions && !ws.has_decisions {
                continue;
            }
            let current = if tab == active {
                r#" active" aria-current="page"#
            } else {
                ""
            };
            html.push_str(&format!(
                r#"<a class="sx-tab{current}" href="{base}{path}" title="{label}"><i class="bi {icon}"></i><span>{label}</span></a>"#
            ));
        }
        html.push_str("</nav>");

        html.push_str(r#"<div class="sx-appbar-end">"#);
        html.push_str(FIND_BUTTON);
        html.push_str(&more_menu(&format!(
            r#"<a href="{base}/print"><i class="bi bi-printer"></i>Print all views</a><a href="{base}/canvas"><i class="bi bi-display"></i>Canvas demo</a><a href="/docs/"><i class="bi bi-book"></i>Documentation</a>"#
        )));
    } else {
        html.push_str(r#"<div class="sx-appbar-end">"#);
        html.push_str(&more_menu(
            r#"<a href="/docs/"><i class="bi bi-book"></i>Documentation</a>"#,
        ));
    }

    html.push_str("</div></header>");
    html
}

/// The app bar for a static `export-viewer` bundle: there is no server, so no
/// index to go back to and none of the server-backed tabs.
pub fn static_app_header(title: &str) -> String {
    format!(
        r#"<header class="sx-appbar" data-catalog="catalog.json"><span class="sx-brand">{BRAND}</span><span class="sx-workspace">{}</span><div class="sx-appbar-end">{FIND_BUTTON}{}</div></header>"#,
        escape(title),
        // The menu still has its separator before the theme; with nothing
        // above it, that would be a stray line.
        more_menu("").replace(r#"<div class="sx-menu-sep"></div>"#, "")
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(has_decisions: bool) -> WorkspaceHeader<'static> {
        WorkspaceHeader {
            slug: "shop",
            title: "Shop & Co",
            has_decisions,
        }
    }

    #[test]
    fn marks_only_the_active_tab() {
        let html = app_header(Some(&ws(false)), Tab::Review);
        assert_eq!(html.matches("aria-current=\"page\"").count(), 1);
        assert!(html.contains(
            r#"class="sx-tab active" aria-current="page" href="/workspace/shop/review""#
        ));
    }

    #[test]
    fn offers_decisions_only_when_there_are_some() {
        assert!(!app_header(Some(&ws(false)), Tab::Diagrams).contains("/decisions"));
        assert!(app_header(Some(&ws(true)), Tab::Diagrams).contains("/workspace/shop/decisions"));
    }

    #[test]
    fn escapes_the_workspace_title() {
        assert!(app_header(Some(&ws(false)), Tab::None).contains("Shop &amp; Co"));
        assert!(static_app_header("<x>").contains("&lt;x&gt;"));
    }

    #[test]
    fn index_has_no_tabs() {
        let html = app_header(None, Tab::None);
        assert!(!html.contains("sx-tabs"));
        // Nothing to search without a workspace.
        assert!(!html.contains("sx-find") && !html.contains("data-workspace"));
        assert!(html.contains("themeToggle"));
    }

    #[test]
    fn static_header_links_nowhere_server_backed() {
        let html = static_app_header("Shop");
        assert!(!html.contains("href"));
        assert!(html.contains(r#"data-catalog="catalog.json""#));
    }
}
