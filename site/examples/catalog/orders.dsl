orders = softwareSystem "Orders" "Owns order placement and history" {
    ordersApi = container "Orders API" "Handles order requests" "Rust"
    ordersDb = container "Database" "Stores order data" "PostgreSQL" { tags "Database" }
    ordersApi -> ordersDb "reads and writes" { kind sync }
}
