workspace "Shop with Decisions" {
    !adrs decisions
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            web = container "Web App" "Storefront" "TypeScript"
            api = container "API" "Handles requests" "Rust"
            db = container "Database" "Stores data" "PostgreSQL" { tags "Database" }
            web -> api "calls"
            api -> db "reads and writes" { kind sync }
        }
        customer -> web "shops on"
    }
    views {
        auto
    }
}
