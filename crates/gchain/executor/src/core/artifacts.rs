//! The artifacts on hand for each link and stage, backed by the store's
//! record of them.
//!
//! An [`ArtifactCache`] only knows what's there; this keeps it in step with
//! the store it's handed, which has to be the same one every time.  A link
//! is *loaded* once the store has been consulted for it,
//! and from then on the cache holds everything that still counts for the
//! link, with a slot for each stored record it passed over, and every
//! artifact recorded goes to the store before it's kept on hand.  That's
//! what makes a read authoritative, so reading a link that isn't loaded is a
//! bug, not a miss.
//!
//! What a stored record counts for is not the table's to say: it's taken in
//! through a [`RecordIntake`], which is how the stages' judgement of their
//! own old artifacts gets applied without the table knowing about stages.
//! Likewise nothing here calls into a stage; what the stages have to do
//! about an artifact going is for the caller to see to first.

use std::collections::HashSet;
use std::sync::Arc;

use strata_gchain_types::*;

use crate::artifact_cache::{ArtifactCache, ArtifactView};
use crate::errors::{GExecError, GExecResult};
use crate::store::{ArtifactRecord, ExecutorStore};

use super::stage::ProducedArtifact;

/// How stored records are turned back into the artifacts they stand for,
/// which is for whoever owns the stages to say.
pub(super) trait RecordIntake<S: GChainSpec> {
    /// Whether taking in a record needs its link's header.
    fn check_needs_header(&self, record: &ArtifactRecord<S>) -> bool;

    /// The artifact a record stands for, if it still counts for its stage;
    /// `None` passes the record over.
    fn take_in(
        &self,
        record: &ArtifactRecord<S>,
        header: Option<&LinkHeader<S>>,
    ) -> GExecResult<Option<Arc<dyn DynProcArtifact>>>;
}

pub(crate) struct ArtifactTable<S: GChainSpec> {
    /// Holds exactly the links the store has been consulted for, with a slot
    /// for every record the store has for each.
    cache: ArtifactCache<S>,
}

impl<S: GChainSpec> ArtifactTable<S> {
    /// Sets up the table with nothing loaded yet.
    pub(super) fn new() -> Self {
        Self {
            cache: ArtifactCache::new(),
        }
    }

    /// The artifacts on hand, for the links loaded.  This is the view stages
    /// process links against.
    pub(super) fn cache(&self) -> &ArtifactCache<S> {
        &self.cache
    }

    // ---- loading -----------------------------------------------------------

    /// Whether the store has been consulted for a link.
    pub(super) fn check_loaded(&self, lref: &LinkRef<S>) -> bool {
        self.cache.contains_link(lref)
    }

    /// Whether the store holds any record for a link, without loading it.
    ///
    /// For a loaded link this counts records that were passed over, which
    /// sit in the store until the link is forgotten and have to be found by
    /// whatever sweeps it.
    pub(super) fn check_present(
        &self,
        store: &impl ExecutorStore<Spec = S>,
        lref: &LinkRef<S>,
    ) -> GExecResult<bool> {
        match self.check_loaded(lref) {
            true => Ok(self.cache.has_entries(lref)),
            false => store.has_link_artifacts(lref).map_err(GExecError::Storage),
        }
    }

    /// Loads a link's stored artifacts if they aren't loaded already, after
    /// which the link counts as loaded whether or not the store had any.
    ///
    /// The link's header is only fetched from the provider if the intake
    /// asks for it for some record.
    pub(super) fn hydrate_link(
        &mut self,
        store: &impl ExecutorStore<Spec = S>,
        provider: &impl ChainProvider<Spec = S>,
        intake: &impl RecordIntake<S>,
        lref: &LinkRef<S>,
    ) -> GExecResult<()> {
        if self.check_loaded(lref) {
            return Ok(());
        }

        let records = store
            .load_link_artifacts(lref)
            .map_err(GExecError::Storage)?;
        let needs_header = records
            .iter()
            .any(|record| intake.check_needs_header(record));
        let header = match needs_header {
            true => provider.fetch_link_header(lref)?,
            false => None,
        };

        // Nothing goes in until every record has been judged, so a record
        // that can't be taken in leaves the link unloaded rather than half.
        let mut entries = Vec::with_capacity(records.len());
        for record in &records {
            let artifact = intake.take_in(record, header.as_ref())?;
            entries.push((record.proc_id(), artifact));
        }

        self.cache.insert_link(lref.clone());
        for (proc_id, artifact) in entries {
            self.cache.insert_entry(lref.clone(), proc_id, artifact);
        }
        Ok(())
    }

    /// Records an artifact a stage produced for a loaded link, persisted
    /// before it's kept on hand.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded, since nothing runs a stage on
    /// such a link.
    pub(super) fn record(
        &mut self,
        store: &impl ExecutorStore<Spec = S>,
        produced: ProducedArtifact<S>,
    ) -> GExecResult<()> {
        let lref = produced.lref();
        self.assert_loaded(lref);
        store
            .store_artifact(produced.record())
            .map_err(GExecError::Storage)?;

        let (artifact, record) = produced.into_parts();
        let (lref, proc_id, _) = record.into_parts();
        self.cache.insert_artifact(lref, proc_id, artifact);
        Ok(())
    }

    /// Forgets a loaded link everywhere: the artifacts on hand go, and so
    /// does every record the store has for it.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded, since whatever has to happen
    /// to them first couldn't have.
    pub(super) fn forget(
        &mut self,
        store: &impl ExecutorStore<Spec = S>,
        lref: &LinkRef<S>,
    ) -> GExecResult<()> {
        self.assert_loaded(lref);
        self.cache.remove_link(lref);
        store
            .discard_link_artifacts(lref)
            .map_err(GExecError::Storage)
    }

    /// Drops what's loaded for every link outside a set, leaving the store
    /// as it is.  The links read as never loaded from then on.
    pub(super) fn evict_to(&mut self, keep: &HashSet<LinkRef<S>>) {
        self.cache.retain_links(keep);
    }

    // ---- what's on hand ----------------------------------------------------

    /// The artifact a stage has for a loaded link.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded.
    pub(super) fn get_artifact<A: ProcArtifact>(
        &self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> Option<Arc<A>> {
        self.assert_loaded(lref);
        self.cache.get_artifact(lref, proc_id)
    }

    /// The artifact a stage has for a loaded link, as the stage made it.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded.
    pub(super) fn get_artifact_dyn(
        &self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> Option<&Arc<dyn DynProcArtifact>> {
        self.assert_loaded(lref);
        self.cache.get_artifact_dyn(lref, proc_id)
    }

    /// Whether a stage covers a loaded link: it has an artifact for it that
    /// doesn't say the link is invalid.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded.
    pub(super) fn covers(&self, lref: &LinkRef<S>, proc_id: ProcId) -> bool {
        self.get_artifact_dyn(lref, proc_id)
            .is_some_and(|a| a.is_link_valid())
    }

    // ---- internals ---------------------------------------------------------

    /// Anything that reads or discards a link's artifacts has to have them
    /// loaded first, or it would silently act on none.
    fn assert_loaded(&self, lref: &LinkRef<S>) {
        assert!(
            self.check_loaded(lref),
            "gchain: acting on a link that isn't loaded ({lref:?})"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::core::pipeline::StagePipeline;
    use crate::mem_store::MemExecutorStore;
    use crate::test_support::*;

    fn id(s: &str) -> ProcId {
        ProcId::from_str(s).expect("test: parse ProcId")
    }

    /// A table with what it's loaded from and what it loads through.
    struct Fixture {
        table: ArtifactTable<TestSpec>,
        store: MemExecutorStore<TestSpec>,
        provider: TestProvider,
        pipeline: StagePipeline<TestSpec>,
    }

    impl Fixture {
        /// Over a store holding the records given as `(lref, proc, version)`,
        /// with the links' headers known to the provider.
        fn new(procs: Vec<TestProc>, records: &[(u8, &str, u32)]) -> Self {
            let store = MemExecutorStore::new();
            let provider = TestProvider::new();
            for (lref, proc, version) in records {
                let data = ProcessorArtifactData::from_artifact(
                    ProcVersion::from(*version),
                    &FlagArtifact(true),
                )
                .expect("test: encode artifact");
                store
                    .store_artifact(&ArtifactRecord::new(TestRef(*lref), id(proc), data))
                    .expect("test: store artifact");
                provider.add_link(*lref, 1, 2);
            }
            Self {
                table: ArtifactTable::new(),
                store,
                provider,
                pipeline: pipeline_of(procs),
            }
        }

        fn hydrate(&mut self, lref: u8) -> GExecResult<()> {
            self.table
                .hydrate_link(&self.store, &self.provider, &self.pipeline, &TestRef(lref))
        }

        fn load(&mut self, lrefs: impl IntoIterator<Item = u8>) {
            for lref in lrefs {
                self.hydrate(lref).expect("test: hydrate link");
            }
        }

        fn present(&self, lref: u8) -> bool {
            self.table
                .check_present(&self.store, &TestRef(lref))
                .expect("test: present")
        }
    }

    #[test]
    fn test_stored_artifacts_count_only_at_the_current_version() {
        let mut f = Fixture::new(
            vec![TestProc::new(), TestProc::new()],
            &[(10, "a", 1), (12, "gone", 1), (11, "a", 2)],
        );
        f.load([10, 12]);

        assert!(f.table.covers(&TestRef(10), id("a")));
        assert!(!f.table.covers(&TestRef(10), id("b")));
        assert!(!f.table.covers(&TestRef(12), id("a")));

        let err = f.hydrate(11).unwrap_err();
        assert!(matches!(err, GExecError::ArtifactFromNewerVersion { .. }));
        assert!(!f.table.check_loaded(&TestRef(11)));
    }

    /// A link the store has only passed-over records for is still present
    /// in the store, which is what sweeping it goes by.
    #[test]
    fn test_presence_is_the_stores_not_the_caches() {
        let mut f = Fixture::new(vec![TestProc::new()], &[(12, "gone", 1)]);

        assert!(f.present(12));
        assert!(!f.present(13));

        f.load([12, 13]);
        assert!(!f.table.covers(&TestRef(12), id("a")));
        assert!(f.present(12));
        assert!(!f.present(13));

        f.table
            .forget(&f.store, &TestRef(12))
            .expect("test: forget");
        assert!(!f.table.check_loaded(&TestRef(12)));
        assert!(!f.present(12));
    }

    #[test]
    fn test_older_artifacts_count_where_the_stage_accepts_them() {
        let proc = TestProc::new().with_version(2).accepting_old([10]);
        let mut f = Fixture::new(vec![proc], &[(10, "a", 1), (11, "a", 1)]);
        f.load([10, 11]);

        assert!(f.table.covers(&TestRef(10), id("a")));
        assert!(!f.table.covers(&TestRef(11), id("a")));
        assert_eq!(
            stored_artifact(&f.store, TestRef(11), id("a")).map(|data| data.exec_version()),
            Some(ProcVersion::from(1))
        );
    }

    /// What's recorded is in the store before it's on hand, and a link
    /// evicted reads as never loaded, so the store is what it comes back
    /// from.
    #[test]
    fn test_recorded_artifacts_come_back_from_the_store_after_eviction() {
        let mut f = Fixture::new(vec![TestProc::new()], &[]);
        f.provider.add_link(10, 1, 2);
        f.load([10]);
        assert!(!f.present(10));

        let produced = f
            .pipeline
            .get_stage(id("a"))
            .expect("test: stage")
            .process(
                &TestRef(10),
                &TestLink(10),
                &ArtifactCache::new(),
                &LinkPath::new_at(TestRef(1)),
            )
            .expect("test: process");
        f.table.record(&f.store, produced).expect("test: record");
        assert!(f.present(10));

        f.table.evict_to(&HashSet::new());
        assert!(!f.table.check_loaded(&TestRef(10)));
        assert!(f.present(10));
        f.load([10]);
        assert!(f.table.covers(&TestRef(10), id("a")));
    }
}
