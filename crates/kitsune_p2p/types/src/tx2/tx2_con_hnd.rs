use crate::codec::*;
use crate::tx2::tx2_adapter::Uniq;
use crate::tx2::tx2_pool::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::BoxFuture;
use futures::future::{FutureExt, TryFutureExt};
use futures::stream::Stream;

use super::tx2_api::AsTx2ConHnd;

#[derive(Clone)]
pub struct TestTx2ConHnd<C: CodecBound> {
    uniq: Uniq,
}

impl<C: CodecBound> std::fmt::Debug for TestTx2ConHnd<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Tx2ConHnd").field(&self.uniq).finish()
    }
}

impl<C: CodecBound> TestTx2ConHnd<C> {
    fn new() -> Self {
        Self {
            uniq: Uniq::default(),
        }
    }
}

impl<C: CodecBound> PartialEq for TestTx2ConHnd<C> {
    fn eq(&self, oth: &Self) -> bool {
        self.con.uniq().eq(&oth.con.uniq())
    }
}

impl<C: CodecBound> Eq for TestTx2ConHnd<C> {}

impl<C: CodecBound> std::hash::Hash for TestTx2ConHnd<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.con.uniq().hash(state)
    }
}

impl<C: CodecBound> AsTx2ConHnd for TestTx2ConHnd<C> {
    type Codec = C;

    fn uniq(&self) -> Uniq {
        todo!()
    }

    fn peer_addr(&self) -> KitsuneResult<TxUrl> {
        todo!()
    }

    fn peer_cert(&self) -> Tx2Cert {
        todo!()
    }

    fn is_closed(&self) -> bool {
        todo!()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        todo!()
    }

    fn notify(
        &self,
        data: &Self::Codec,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        todo!()
    }

    fn request(
        &self,
        data: &Self::Codec,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Self::Codec>> {
        todo!()
    }
}
