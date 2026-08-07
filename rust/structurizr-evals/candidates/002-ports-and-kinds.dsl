workspace "Library" {
    model {
        reader = person "Reader"
        library = softwareSystem "Library" {
            web = container "Web Portal" "Search and borrow books" "TypeScript"
            api = container "Catalog API" "Handles catalog and loan requests" "Rust" {
                status implemented
                port catalog "Catalog REST API" { protocol "HTTPS/JSON" }
            }
            db = container "Database" "Stores books and loans" "PostgreSQL" { tags "Database" }
            web -> api.catalog "calls"
            api -> db "reads and writes" { kind sync }
        }
        reader -> web "browses on"
    }
    views {
        auto
    }
}
