pub mod call_zome_workspace_lock;

pub struct CallZomeWorkspace {
    pub source_chain: SourceChain,
    pub meta_authored: MetadataBuf<AuthoredPrefix>,
    pub element_integrated: ElementBuf<IntegratedPrefix>,
    pub meta_integrated: MetadataBuf<IntegratedPrefix>,
    pub element_cache: ElementBuf,
    pub meta_cache: MetadataBuf,
}

impl<'a> CallZomeWorkspace {
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let source_chain = SourceChain::new(env.clone())?;
        let meta_authored = MetadataBuf::authored(env.clone())?;
        let element_integrated = ElementBuf::vault(env.clone(), true)?;
        let meta_integrated = MetadataBuf::vault(env.clone())?;
        let element_cache = ElementBuf::cache(env.clone())?;
        let meta_cache = MetadataBuf::cache(env)?;

        Ok(CallZomeWorkspace {
            source_chain,
            meta_authored,
            element_integrated,
            meta_integrated,
            element_cache,
            meta_cache,
        })
    }

    pub fn cascade(&'a mut self, network: HolochainP2pCell) -> Cascade<'a> {
        Cascade::new(
            self.source_chain.env().clone(),
            &self.source_chain.elements(),
            &self.meta_authored,
            &self.element_integrated,
            &self.meta_integrated,
            &mut self.element_cache,
            &mut self.meta_cache,
            network,
        )
    }

    /// Cascade without a network connection
    pub fn cascade_local(&'a mut self) -> Cascade<'a> {
        let authored_data = DbPair::new(&self.source_chain.elements(), &self.meta_authored);
        let cache_data = DbPairMut::new(&mut self.element_cache, &mut self.meta_cache);
        let integrated_data = DbPair::new(&self.element_integrated, &self.meta_integrated);
        Cascade::empty()
            .with_authored(authored_data)
            .with_cache(cache_data)
            .with_integrated(integrated_data)
    }

    pub fn env(&self) -> &EnvironmentRead {
        self.meta_authored.env()
    }
}

impl Workspace for CallZomeWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn_ref(writer)?;
        self.meta_authored.flush_to_txn_ref(writer)?;
        self.element_cache.flush_to_txn_ref(writer)?;
        self.meta_cache.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
