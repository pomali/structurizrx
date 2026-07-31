workspace "Shop" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            web = container "Web App" "Storefront" "TypeScript"
            api = container "API" "Handles requests" "Rust" {
                status implemented
                port rest "Customer REST API" { protocol "HTTPS/JSON" }
            }
            db = container "Database" "Stores data" "PostgreSQL" { tags "Database" }
            web -> api.rest "calls"
            api -> db "reads and writes" { kind sync }
        }
        customer -> web "shops on"
    }
    views {
        auto
        auto focus api
        auto lint
    }
}
