use canic_cdk::query;

use crate::contract::{CanisterEndpointView, required_endpoint_views};

#[query]
fn get_canister_endpoint_inventory() -> Vec<CanisterEndpointView> {
    required_endpoint_views()
}
