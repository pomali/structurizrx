workspace "Shop Billing" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" "Sells goods to customers"
        billing = softwareSystem "Billing" "Charges customers for completed orders"
        erp = softwareSystem "ERP" "Tracks finance and inventory" { tags "External" }
        customer -> shop "buys things from"
        shop -> billing "requests a charge for the order total from"
        billing -> erp "posts the settled charge to"
    }
    views {
        auto
    }
}
