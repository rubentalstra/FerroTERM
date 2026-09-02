//! Edition identification from the module dependency reference set.
//!
//! A release assembles modules; each module dependency member says that its
//! module, at `sourceEffectiveTime`, depends on the referenced module at
//! `targetEffectiveTime`. The edition is identified by a module, and its
//! version URI is `http://snomed.info/sct/[moduleId]/version/[YYYYMMDD]`
//! (<https://docs.snomed.org/snomed-ct-specifications/snomed-ct-uri-standard>).

use std::collections::{BTreeMap, BTreeSet};

use crate::id::{ConceptId, ModuleId};
use crate::refset::ModuleDependencyMember;
use crate::time::EffectiveTime;

/// The edition could not be identified.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditionError {
    /// No active module dependency member was given.
    #[error("no active module dependency members")]
    NoDependencies,
    /// A member names a target module that is not a concept identifier.
    #[error("module dependency member {member} names {target}, which is not a concept identifier")]
    MalformedTarget {
        /// The member.
        member: String,
        /// The offending target.
        target: String,
    },
    /// Several modules depend on others without being depended on, and none
    /// of them carries the release date.
    #[error("several root modules and none carries the release date {release}: {roots:?}")]
    AmbiguousRoot {
        /// The release date.
        release: String,
        /// The candidate modules.
        roots: Vec<String>,
    },
}

/// The identified edition of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edition {
    /// The edition's module: the root module carrying the release date.
    pub module: ModuleId,
    /// Other root modules the release ships beside the edition module (a
    /// sibling such as a mapping module), which the edition URI does not cover.
    pub sibling_roots: Vec<ModuleId>,
    /// The edition's version.
    pub effective_time: EffectiveTime,
    /// Every module in the release with the version it is at.
    pub modules: BTreeMap<ModuleId, EffectiveTime>,
}

impl Edition {
    /// Identifies the edition from the active module dependency members.
    ///
    /// The root modules are those that depend on others and are depended on
    /// by none. With one root it is the edition; with several, the one whose
    /// `sourceEffectiveTime` equals `release` is, and the others are reported
    /// as `sibling_roots`.
    ///
    /// The specifications define an edition by its focus module, the module
    /// dependent on every other module
    /// (<https://docs.snomed.org/snomed-ct-practical-guides/snomed-ct-extension-guide/4-logical-design/4.4-editions>),
    /// and give no consumer-side rule for a release with several roots; the
    /// date rule is our own design for packages such as the NL edition, which
    /// ships the ICD-10 mapping module as a sibling root.
    ///
    /// # Errors
    ///
    /// Returns [`EditionError`] when no members are given, a target is not a
    /// concept identifier, or the root is ambiguous.
    pub fn identify(
        members: &[ModuleDependencyMember],
        release: EffectiveTime,
    ) -> Result<Self, EditionError> {
        let active: Vec<&ModuleDependencyMember> =
            members.iter().filter(|m| m.member.active).collect();
        if active.is_empty() {
            return Err(EditionError::NoDependencies);
        }
        let mut modules: BTreeMap<ModuleId, EffectiveTime> = BTreeMap::new();
        let mut depended_on: BTreeSet<ModuleId> = BTreeSet::new();
        for member in &active {
            let source = member.member.module_id;
            modules
                .entry(source)
                .and_modify(|t| *t = (*t).max(member.source_effective_time))
                .or_insert(member.source_effective_time);
            let target = ConceptId::try_from(member.member.referenced_component_id)
                .map(ModuleId::from)
                .map_err(|_| EditionError::MalformedTarget {
                    member: member.member.id.to_string(),
                    target: member.member.referenced_component_id.to_string(),
                })?;
            depended_on.insert(target);
            modules
                .entry(target)
                .and_modify(|t| *t = (*t).max(member.target_effective_time))
                .or_insert(member.target_effective_time);
        }
        let roots: Vec<ModuleId> = modules
            .keys()
            .filter(|m| !depended_on.contains(*m))
            .copied()
            .collect();
        let module = match roots.as_slice() {
            [only] => *only,
            many => *many
                .iter()
                .find(|m| modules.get(*m) == Some(&release))
                .ok_or_else(|| EditionError::AmbiguousRoot {
                    release: release.compact(),
                    roots: many.iter().map(ToString::to_string).collect(),
                })?,
        };
        let effective_time = modules.get(&module).copied().unwrap_or(release);
        let sibling_roots = roots.into_iter().filter(|m| *m != module).collect();
        Ok(Self {
            module,
            sibling_roots,
            effective_time,
            modules,
        })
    }

    /// The edition URI, `http://snomed.info/sct/[moduleId]`.
    #[must_use]
    pub fn edition_uri(&self) -> String {
        format!("http://snomed.info/sct/{}", self.module)
    }

    /// The version URI, `http://snomed.info/sct/[moduleId]/version/[YYYYMMDD]`.
    #[must_use]
    pub fn version_uri(&self) -> String {
        format!(
            "{}/version/{}",
            self.edition_uri(),
            self.effective_time.compact()
        )
    }
}
