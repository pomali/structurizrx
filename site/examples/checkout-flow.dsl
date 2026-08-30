workspace "Checkout Flow" {
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
        dynamic shop "checkout" "Placing an order" {
            customer -> web "Clicks 'Buy now'"
            web -> api "POST /orders"
            api -> db "Inserts order row"
            api -> web "201 Created"
            web -> customer "Shows order confirmation"
            autoLayout
        }
    }
}
