//! Identifiers the SNOMED CT specifications publish, as typed constants.
//!
//! Every value here is a published SCTID from the release file specification,
//! the concept model, or the metadata hierarchy; a test checks each one's
//! check digit and partition, and nothing in this module is invented.

use crate::id::{ConceptId, RefsetId, Sctid};

/// `116680003 |Is a (attribute)|`, the subtype relationship.
pub const IS_A: ConceptId = ConceptId::published(116_680_003);

/// `900000000000003001 |Fully specified name|`.
pub const FULLY_SPECIFIED_NAME: ConceptId = ConceptId::published(900_000_000_000_003_001);
/// `900000000000013009 |Synonym|`.
pub const SYNONYM: ConceptId = ConceptId::published(900_000_000_000_013_009);
/// `900000000000550004 |Definition|`, the text definition type.
pub const DEFINITION: ConceptId = ConceptId::published(900_000_000_000_550_004);

/// `900000000000548007 |Preferred|` acceptability.
pub const PREFERRED: ConceptId = ConceptId::published(900_000_000_000_548_007);
/// `900000000000549004 |Acceptable|` acceptability.
pub const ACCEPTABLE: ConceptId = ConceptId::published(900_000_000_000_549_004);

/// `900000000000074008 |Primitive|` definition status.
pub const PRIMITIVE: ConceptId = ConceptId::published(900_000_000_000_074_008);
/// `900000000000073002 |Defined|` definition status.
pub const DEFINED: ConceptId = ConceptId::published(900_000_000_000_073_002);

/// `900000000000011006 |Inferred relationship|`.
pub const INFERRED: ConceptId = ConceptId::published(900_000_000_000_011_006);
/// `900000000000010007 |Stated relationship|`.
pub const STATED: ConceptId = ConceptId::published(900_000_000_000_010_007);
/// `900000000000227009 |Additional relationship|`.
pub const ADDITIONAL: ConceptId = ConceptId::published(900_000_000_000_227_009);

/// `900000000000207008 |SNOMED CT core module|`.
pub const CORE_MODULE: ConceptId = ConceptId::published(900_000_000_000_207_008);
/// `900000000000012004 |SNOMED CT model component module|`.
pub const MODEL_COMPONENT_MODULE: ConceptId = ConceptId::published(900_000_000_000_012_004);

/// `900000000000534007 |Module dependency reference set|`.
pub const MODULE_DEPENDENCY_REFSET: RefsetId = RefsetId::published(900_000_000_000_534_007);
/// `900000000000456007 |Reference set descriptor reference set|`.
pub const REFSET_DESCRIPTOR_REFSET: RefsetId = RefsetId::published(900_000_000_000_456_007);
/// `733073007 |OWL axiom reference set|`.
pub const OWL_AXIOM_REFSET: RefsetId = RefsetId::published(733_073_007);
/// `900000000000508004 |GB English language reference set|`.
pub const GB_ENGLISH_LANGUAGE_REFSET: RefsetId = RefsetId::published(900_000_000_000_508_004);
/// `900000000000509007 |US English language reference set|`.
pub const US_ENGLISH_LANGUAGE_REFSET: RefsetId = RefsetId::published(900_000_000_000_509_007);

/// The reference set descriptor attribute types.
pub mod attribute_type {
    use crate::id::ConceptId;

    /// `900000000000460005 |Component type|`.
    pub const COMPONENT: ConceptId = ConceptId::published(900_000_000_000_460_005);
    /// `900000000000461009 |Concept type component|`.
    pub const CONCEPT: ConceptId = ConceptId::published(900_000_000_000_461_009);
    /// `900000000000462002 |Description type component|`.
    pub const DESCRIPTION: ConceptId = ConceptId::published(900_000_000_000_462_002);
    /// `900000000000463007 |Relationship type component|`.
    pub const RELATIONSHIP: ConceptId = ConceptId::published(900_000_000_000_463_007);
    /// `900000000000464001 |Reference set member type component|`.
    pub const MEMBER: ConceptId = ConceptId::published(900_000_000_000_464_001);
    /// `900000000000465000 |String|`.
    pub const STRING: ConceptId = ConceptId::published(900_000_000_000_465_000);
    /// `900000000000466004 |Time|`.
    pub const TIME: ConceptId = ConceptId::published(900_000_000_000_466_004);
    /// `900000000000467008 |Integer|`.
    pub const INTEGER: ConceptId = ConceptId::published(900_000_000_000_467_008);
    /// `900000000000468003 |Unsigned integer|`.
    pub const UNSIGNED_INTEGER: ConceptId = ConceptId::published(900_000_000_000_468_003);
    /// `900000000000469006 |Signed integer|`.
    pub const SIGNED_INTEGER: ConceptId = ConceptId::published(900_000_000_000_469_006);
}

/// Every constant, for the validation test.
#[must_use]
pub fn all() -> Vec<Sctid> {
    vec![
        IS_A.sctid(),
        FULLY_SPECIFIED_NAME.sctid(),
        SYNONYM.sctid(),
        DEFINITION.sctid(),
        PREFERRED.sctid(),
        ACCEPTABLE.sctid(),
        PRIMITIVE.sctid(),
        DEFINED.sctid(),
        INFERRED.sctid(),
        STATED.sctid(),
        ADDITIONAL.sctid(),
        CORE_MODULE.sctid(),
        MODEL_COMPONENT_MODULE.sctid(),
        MODULE_DEPENDENCY_REFSET.concept().sctid(),
        REFSET_DESCRIPTOR_REFSET.concept().sctid(),
        OWL_AXIOM_REFSET.concept().sctid(),
        GB_ENGLISH_LANGUAGE_REFSET.concept().sctid(),
        US_ENGLISH_LANGUAGE_REFSET.concept().sctid(),
        attribute_type::COMPONENT.sctid(),
        attribute_type::CONCEPT.sctid(),
        attribute_type::DESCRIPTION.sctid(),
        attribute_type::RELATIONSHIP.sctid(),
        attribute_type::MEMBER.sctid(),
        attribute_type::STRING.sctid(),
        attribute_type::TIME.sctid(),
        attribute_type::INTEGER.sctid(),
        attribute_type::UNSIGNED_INTEGER.sctid(),
        attribute_type::SIGNED_INTEGER.sctid(),
    ]
}

#[cfg(test)]
mod tests {
    use crate::id::{Partition, Sctid};

    #[test]
    fn every_published_constant_is_a_valid_concept_identifier() {
        for id in super::all() {
            let parsed = Sctid::parse(&id.to_string()).expect("published SCTIDs validate");
            assert_eq!(parsed, id);
            assert_eq!(parsed.partition(), Ok(Partition::Concept));
        }
    }
}
