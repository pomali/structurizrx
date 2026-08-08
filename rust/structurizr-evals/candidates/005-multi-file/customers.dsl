customers = softwareSystem "Customers" "Owns customer profiles and accounts" {
    customersApi = container "Customers API" "Handles profile requests" "Rust"
    customersDb = container "Database" "Stores customer data" "PostgreSQL" { tags "Database" }
    customersApi -> customersDb "reads and writes" { kind sync }
}
