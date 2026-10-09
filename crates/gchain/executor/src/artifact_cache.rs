use std::collections::*;
use std::sync::Arc;

use strata_gchain_types::*;

/// Read access to the artifacts stages have for links, which is what a stage
/// draws its deps from while it's run.
///
/// Running a stage only ever reads artifacts, so it takes this rather than
/// any one way of holding them.
pub trait ArtifactView<S: GChainSpec> {
    /// Gets the type-erased artifact some processor stage has for a link.
    fn get_artifact_dyn(
        &self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> Option<&Arc<dyn DynProcArtifact>>;

    /// Gets the artifact some processor stage has for a link, downcast to its
    /// concrete type.
    ///
    /// Returns `None` if the stage has no artifact for the link, or if the
    /// artifact it has isn't of type `A`.
    fn get_artifact<A: ProcArtifact>(&self, lref: &LinkRef<S>, proc_id: ProcId) -> Option<Arc<A>>
    where
        Self: Sized,
    {
        let artifact = self.get_artifact_dyn(lref, proc_id)?;
        artifact.clone().into_any_arc().downcast::<A>().ok()
    }
}

/// A borrowed view is a view, which is also what lets a type-erased one be
/// handed to code that's generic over the view.
impl<S: GChainSpec, V: ArtifactView<S> + ?Sized> ArtifactView<S> for &V {
    fn get_artifact_dyn(
        &self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> Option<&Arc<dyn DynProcArtifact>> {
        (**self).get_artifact_dyn(lref, proc_id)
    }
}

/// The artifacts on hand for the links that have been taken in.
///
/// Artifacts are keyed by the processor stage that produced them, since that's
/// how processor stages name their dependencies (see [`ProcDeps`]).  Multiple
/// stages may produce artifacts of the same concrete type.
///
/// A link is in the cache once it's been taken in, whether or not anything
/// was found for it, and each stage that had something for it gets a slot,
/// holding the artifact if what the stage had still counts as one.  So the
/// cache can say which links it speaks for and which of those some stage
/// has something for, without that being kept anywhere else.
pub struct ArtifactCache<S: GChainSpec> {
    links: HashMap<LinkRef<S>, LinkSlots>,
}

/// A slot per stage that has something for a link, holding its artifact if
/// what it has counts as one.
type LinkSlots = BTreeMap<ProcId, Option<Arc<dyn DynProcArtifact>>>;

impl<S: GChainSpec> ArtifactCache<S> {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
        }
    }

    /// Whether a link has been taken in.
    pub fn contains_link(&self, lref: &LinkRef<S>) -> bool {
        self.links.contains_key(lref)
    }

    /// Whether some stage has something for a link that's been taken in,
    /// counting slots that hold no artifact.
    pub fn has_entries(&self, lref: &LinkRef<S>) -> bool {
        self.links.get(lref).is_some_and(|atbl| !atbl.is_empty())
    }

    /// Takes in a link, with no artifacts for it yet.
    pub fn insert_link(&mut self, lref: LinkRef<S>) {
        self.links.entry(lref).or_default();
    }

    /// Stores the artifact a processor stage produced for a link, replacing
    /// anything that stage had stored for it.
    pub fn insert_artifact(
        &mut self,
        lref: LinkRef<S>,
        proc_id: ProcId,
        artifact: Arc<dyn DynProcArtifact>,
    ) {
        self.insert_entry(lref, proc_id, Some(artifact));
    }

    /// Takes note of what a processor stage has for a link, which is an
    /// artifact or just that there was something that no longer counts as
    /// one.  Replaces anything that stage had stored for the link.
    pub fn insert_entry(
        &mut self,
        lref: LinkRef<S>,
        proc_id: ProcId,
        artifact: Option<Arc<dyn DynProcArtifact>>,
    ) {
        self.links
            .entry(lref)
            .or_default()
            .insert(proc_id, artifact);
    }

    /// Drops a link along with everything stored for it.
    pub fn remove_link(&mut self, lref: &LinkRef<S>) {
        self.links.remove(lref);
    }

    /// Drops every link outside the provided set, such as after committing
    /// or abandoning a path.
    pub fn retain_links(&mut self, keep: &HashSet<LinkRef<S>>) {
        self.links.retain(|lref, _| keep.contains(lref));
    }
}

impl<S: GChainSpec> Default for ArtifactCache<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: GChainSpec> ArtifactView<S> for ArtifactCache<S> {
    fn get_artifact_dyn(
        &self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> Option<&Arc<dyn DynProcArtifact>> {
        self.links
            .get(lref)
            .and_then(|atbl| atbl.get(&proc_id))
            .and_then(Option::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::test_support::*;

    fn proc_id(s: &str) -> ProcId {
        ProcId::from_str(s).expect("test: parse ProcId")
    }

    fn count(cache: &impl ArtifactView<TestSpec>, lref: u8, id: &str) -> Option<u32> {
        cache
            .get_artifact::<CountArtifact>(&TestRef(lref), proc_id(id))
            .map(|a| a.0)
    }

    /// Two stages may produce artifacts of the same concrete type, so the stage
    /// that produced an artifact has to be what distinguishes them.
    #[test]
    fn test_get_artifact_downcasts_what_each_stage_inserted() {
        let mut cache = ArtifactCache::<TestSpec>::new();
        cache.insert_artifact(TestRef(1), proc_id("first"), Arc::new(CountArtifact(7)));
        cache.insert_artifact(TestRef(1), proc_id("second"), Arc::new(CountArtifact(9)));

        assert_eq!(count(&cache, 1, "first"), Some(7));
        assert_eq!(count(&cache, 1, "second"), Some(9));
        assert_eq!(count(&cache, 2, "first"), None);
        assert_eq!(count(&cache, 1, "absent"), None);
        assert_eq!(
            cache.get_artifact::<FlagArtifact>(&TestRef(1), proc_id("first")),
            None
        );

        cache.insert_artifact(TestRef(1), proc_id("first"), Arc::new(CountArtifact(8)));
        assert_eq!(count(&cache, 1, "first"), Some(8));
    }

    #[test]
    fn test_removals_drop_only_what_they_name() {
        let mut cache = ArtifactCache::<TestSpec>::new();
        for (lref, id) in [(1, "a"), (1, "b"), (2, "a"), (3, "a")] {
            cache.insert_artifact(
                TestRef(lref),
                proc_id(id),
                Arc::new(CountArtifact(u32::from(lref))),
            );
        }

        cache.remove_link(&TestRef(2));
        assert_eq!(count(&cache, 2, "a"), None);
        assert!(!cache.contains_link(&TestRef(2)));

        cache.retain_links(&HashSet::from([TestRef(1)]));
        assert_eq!(count(&cache, 1, "a"), Some(1));
        assert_eq!(count(&cache, 3, "a"), None);
        assert!(!cache.contains_link(&TestRef(3)));
    }

    /// A link taken in is known even with nothing for it, and a slot
    /// without an artifact counts as something for the link without being
    /// something a stage can read.
    #[test]
    fn test_links_and_empty_slots_are_known_but_not_readable() {
        let mut cache = ArtifactCache::<TestSpec>::new();
        cache.insert_link(TestRef(1));
        cache.insert_entry(TestRef(2), proc_id("a"), None);

        assert!(cache.contains_link(&TestRef(1)));
        assert!(!cache.has_entries(&TestRef(1)));
        assert!(cache.contains_link(&TestRef(2)));
        assert!(cache.has_entries(&TestRef(2)));
        assert!(!cache.contains_link(&TestRef(3)));
        assert_eq!(count(&cache, 2, "a"), None);

        cache.insert_artifact(TestRef(2), proc_id("a"), Arc::new(CountArtifact(5)));
        assert_eq!(count(&cache, 2, "a"), Some(5));
    }

    /// A stage sees the artifacts through a type-erased borrow.
    #[test]
    fn test_view_reads_through_a_type_erased_borrow() {
        let mut cache = ArtifactCache::<TestSpec>::new();
        cache.insert_artifact(TestRef(1), proc_id("a"), Arc::new(CountArtifact(1)));

        let view: &dyn ArtifactView<TestSpec> = &cache;
        assert_eq!(count(&view, 1, "a"), Some(1));
        assert_eq!(count(&view, 2, "a"), None);
    }
}
