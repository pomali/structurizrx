workspace "Shop Layers" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            group "Core" {
                web = container "Web App" "Storefront" "TypeScript"
                api = container "API" "Handles requests" "Rust"
            }
            group "Data" {
                db = container "Database" "Stores data" "PostgreSQL" { tags "Database" }
            }
            web -> api "calls"
            api -> db "reads and writes" { kind sync }
        }
        customer -> web "shops on"
    }
    views {
        auto
        auto layer "Core"
        auto layer "Data"
    }
}
