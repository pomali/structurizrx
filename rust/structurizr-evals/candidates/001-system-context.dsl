workspace "Food Delivery" {
    model {
        customer = person "Customer"
        platform = softwareSystem "Delivery Platform" "Lets customers order food for delivery"
        paymentGateway = softwareSystem "Payment Gateway" "Processes card payments" { tags "External" }
        customer -> platform "places orders using"
        platform -> paymentGateway "processes payments via" { kind sync }
    }
    views {
        auto
    }
}
