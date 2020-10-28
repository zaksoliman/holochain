use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
use holochain_zome_types::GetInput;
use holochain_zome_types::GetOutput;
use kitsune_p2p::FutureTimeoutExt;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetInput,
) -> RibosomeResult<GetOutput> {
    let (hash, options) = input.into_inner();

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    // timeouts must be handled by the network
    // tokio_safe_block_on::tokio_safe_block_on(
    futures::executor::block_on(
        async move {
            dbg!("1");
            // let maybe_element = call_context

            // XXX: THIS is where things freeze up. We see "1" but not "2".
            // This seems to indicate that this task is only ever polled once,
            // since even the 50ms await is not polled (else we would see "2")

            tokio::time::delay_for(std::time::Duration::from_millis(50)).await;
            dbg!("2");
            let mut maybe_element = loop {
                let mut delay = tokio::time::delay_for(std::time::Duration::from_millis(50));
                let fut = call_context
                    .host_access
                    .workspace()
                    .write()
                    .timeout(std::time::Duration::from_secs(2), "getting write lock");
                tokio::select! {
                    _ = &mut delay, if !delay.is_elapsed() => {
                        println!("operation timed out");
                    }
                    r = fut => break r,
                }
            };
            dbg!("3");
            let maybe_element = maybe_element
                .cascade(network)
                .dht_get(hash, options.into())
                .timeout(std::time::Duration::from_secs(2), "dht_get")
                .await?;

            dbg!("4");
            Ok(GetOutput::new(maybe_element))
        },
        // std::time::Duration::from_secs(10),
    )
}

// we are relying on the create tests to show the commit/get round trip
// @see commit_entry.rs
