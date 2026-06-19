use novex_provider_client::CRATE_ID;

#[test]
fn module_describes_provider_client_boundary() {
    assert_eq!(CRATE_ID, "novex-provider-client");
}
