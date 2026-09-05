//! Reference set members.
//!
//! Every reference set file starts with the six member columns; the file
//! name's pattern letters type each additional column, and the header names
//! it. A [`Member`] keeps the additional fields by name and type, and the
//! typed views (language, association, map, module dependency, OWL, and the
//! rest) read the columns the release file specification defines for them.

use std::io::Read;
use std::path::Path;

use crate::file::FieldKind;
use crate::id::{ConceptId, MemberId, ModuleId, RefsetId, Sctid};
use crate::reader::{FieldError, Record, Rf2Error, Rf2Reader};
use crate::time::EffectiveTime;

/// The six columns every reference set file starts with.
pub const MEMBER_COLUMNS: [&str; 6] = [
    "id",
    "effectiveTime",
    "active",
    "moduleId",
    "refsetId",
    "referencedComponentId",
];

/// One additional column's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    /// A component reference.
    Component(Sctid),
    /// An integer.
    Integer(i64),
    /// A string.
    String(String),
}

/// A reference set member with its additional fields by column name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The member.
    pub id: MemberId,
    /// When this version took effect.
    pub effective_time: EffectiveTime,
    /// Whether the member is active.
    pub active: bool,
    /// The module that owns the member.
    pub module_id: ModuleId,
    /// The reference set.
    pub refset_id: RefsetId,
    /// The component the member refers to.
    pub referenced_component_id: Sctid,
    /// The additional columns, in file order.
    pub fields: Vec<(String, FieldValue)>,
}

impl Member {
    /// The additional field named `column`, if the file has it.
    #[must_use]
    pub fn field(&self, column: &str) -> Option<&FieldValue> {
        self.fields
            .iter()
            .find(|(name, _)| name == column)
            .map(|(_, value)| value)
    }

    fn component(&self, column: &str) -> Result<Sctid, ViewError> {
        match self.field(column) {
            Some(FieldValue::Component(id)) => Ok(*id),
            other => Err(ViewError::field(column, other)),
        }
    }

    fn concept(&self, column: &str) -> Result<ConceptId, ViewError> {
        let id = self.component(column)?;
        // The view error names the column and field; the id error adds nothing.
        let Ok(id) = ConceptId::try_from(id) else {
            return Err(ViewError::field(column, self.field(column)));
        };
        Ok(id)
    }

    fn string(&self, column: &str) -> Result<&str, ViewError> {
        match self.field(column) {
            Some(FieldValue::String(text)) => Ok(text),
            other => Err(ViewError::field(column, other)),
        }
    }

    fn integer(&self, column: &str) -> Result<i64, ViewError> {
        match self.field(column) {
            Some(FieldValue::Integer(value)) => Ok(*value),
            other => Err(ViewError::field(column, other)),
        }
    }
}

/// A member does not have the shape a typed view expects.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ViewError {
    /// A column is missing or holds another kind of value.
    #[error("column {column} is {found}")]
    Field {
        /// The column name.
        column: String,
        /// What was found.
        found: String,
    },
    /// A value has the wrong form for the view.
    #[error("column {column}: {source}")]
    Value {
        /// The column name.
        column: String,
        /// The underlying error.
        #[source]
        source: FieldError,
    },
}

impl ViewError {
    fn field(column: &str, found: Option<&FieldValue>) -> Self {
        Self::Field {
            column: column.to_owned(),
            found: match found {
                None => String::from("absent"),
                Some(FieldValue::Component(_)) => String::from("a component reference"),
                Some(FieldValue::Integer(_)) => String::from("an integer"),
                Some(FieldValue::String(_)) => String::from("a string"),
            },
        }
    }
}

/// A streaming iterator over the members of one reference set file.
#[derive(Debug)]
pub struct Members<R: Read> {
    reader: Rf2Reader<R>,
    kinds: Vec<FieldKind>,
}

impl Members<std::io::BufReader<std::fs::File>> {
    /// Opens `path`, whose additional columns are typed by `kinds` (from the
    /// file name); the header must name six member columns plus one per kind.
    ///
    /// # Errors
    ///
    /// Returns [`Rf2Error`] when the file cannot be read or its header has the
    /// wrong number of columns.
    pub fn open(path: &Path, kinds: &[FieldKind]) -> Result<Self, Rf2Error> {
        let file = std::fs::File::open(path).map_err(|source| Rf2Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::new(path, std::io::BufReader::new(file), kinds)
    }
}

impl<R: Read> Members<R> {
    /// Wraps `read`, typing the additional columns by `kinds`.
    ///
    /// # Errors
    ///
    /// Returns [`Rf2Error`] when the header does not start with the member
    /// columns or has a different number of additional columns than `kinds`.
    pub fn new(path: &Path, read: R, kinds: &[FieldKind]) -> Result<Self, Rf2Error> {
        let reader = Rf2Reader::new_with(path, read, |actual| {
            let fixed_ok = actual.len() == MEMBER_COLUMNS.len() + kinds.len()
                && actual.iter().zip(MEMBER_COLUMNS).all(|(a, e)| a == e);
            if fixed_ok {
                Ok(())
            } else {
                let mut expected: Vec<String> =
                    MEMBER_COLUMNS.iter().map(|c| (*c).to_owned()).collect();
                expected.extend(kinds.iter().map(|k| format!("<{k:?}>")));
                Err(Rf2Error::Header {
                    path: path.to_path_buf(),
                    expected,
                    actual: actual.to_vec(),
                })
            }
        })?;
        Ok(Self {
            reader,
            kinds: kinds.to_vec(),
        })
    }

    fn member(kinds: &[FieldKind], record: &Record<'_>) -> Result<Member, Rf2Error> {
        let mut fields = Vec::with_capacity(kinds.len());
        for (offset, kind) in kinds.iter().enumerate() {
            let index = MEMBER_COLUMNS.len() + offset;
            let name = record.column_name(index).unwrap_or_default().to_owned();
            let value = match kind {
                FieldKind::Component => FieldValue::Component(record.parse(index, Sctid::parse)?),
                FieldKind::Integer => FieldValue::Integer(record.integer(index)?),
                FieldKind::String => FieldValue::String(record.text(index)?.to_owned()),
            };
            fields.push((name, value));
        }
        Ok(Member {
            id: record.parse(0, MemberId::parse)?,
            effective_time: record.parse(1, EffectiveTime::parse)?,
            active: record.boolean(2)?,
            module_id: record.parse(3, ModuleId::parse)?,
            refset_id: record.parse(4, RefsetId::parse)?,
            referenced_component_id: record.parse(5, Sctid::parse)?,
            fields,
        })
    }
}

impl<R: Read> Iterator for Members<R> {
    type Item = Result<Member, Rf2Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.next_record() {
            Ok(Some(record)) => Some(Self::member(&self.kinds, &record)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// A language reference set member: `acceptabilityId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageMember {
    /// The member.
    pub member: Member,
    /// Preferred or acceptable.
    pub acceptability_id: ConceptId,
}

impl TryFrom<Member> for LanguageMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        let acceptability_id = member.concept("acceptabilityId")?;
        Ok(Self {
            member,
            acceptability_id,
        })
    }
}

/// An association reference set member: `targetComponentId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationMember {
    /// The member.
    pub member: Member,
    /// The component the referenced component is associated with.
    pub target_component_id: Sctid,
}

impl TryFrom<Member> for AssociationMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        let target_component_id = member.component("targetComponentId")?;
        Ok(Self {
            member,
            target_component_id,
        })
    }
}

/// An attribute value reference set member: `valueId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeValueMember {
    /// The member.
    pub member: Member,
    /// The value concept.
    pub value_id: ConceptId,
}

impl TryFrom<Member> for AttributeValueMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        let value_id = member.concept("valueId")?;
        Ok(Self { member, value_id })
    }
}

/// A simple map reference set member: `mapTarget`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleMapMember {
    /// The member.
    pub member: Member,
    /// The code in the target scheme.
    pub map_target: String,
}

impl TryFrom<Member> for SimpleMapMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        let map_target = member.string("mapTarget")?.to_owned();
        Ok(Self { member, map_target })
    }
}

/// An extended map reference set member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedMapMember {
    /// The member.
    pub member: Member,
    /// The map group.
    pub map_group: i64,
    /// The priority within the group.
    pub map_priority: i64,
    /// The rule that selects this row.
    pub map_rule: String,
    /// Advice to the user.
    pub map_advice: String,
    /// The target code, possibly empty.
    pub map_target: String,
    /// The correlation between source and target.
    pub correlation_id: ConceptId,
    /// The map category.
    pub map_category_id: ConceptId,
}

impl TryFrom<Member> for ExtendedMapMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        Ok(Self {
            map_group: member.integer("mapGroup")?,
            map_priority: member.integer("mapPriority")?,
            map_rule: member.string("mapRule")?.to_owned(),
            map_advice: member.string("mapAdvice")?.to_owned(),
            map_target: member.string("mapTarget")?.to_owned(),
            correlation_id: member.concept("correlationId")?,
            map_category_id: member.concept("mapCategoryId")?,
            member,
        })
    }
}

/// A module dependency reference set member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDependencyMember {
    /// The member; `module_id` is the dependent module, `referenced_component_id` the one it depends on.
    pub member: Member,
    /// The version of the dependent module.
    pub source_effective_time: EffectiveTime,
    /// The version of the module depended on.
    pub target_effective_time: EffectiveTime,
}

impl TryFrom<Member> for ModuleDependencyMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        let time = |column: &str| -> Result<EffectiveTime, ViewError> {
            EffectiveTime::parse(member.string(column)?).map_err(|source| ViewError::Value {
                column: column.to_owned(),
                source: source.into(),
            })
        };
        Ok(Self {
            source_effective_time: time("sourceEffectiveTime")?,
            target_effective_time: time("targetEffectiveTime")?,
            member,
        })
    }
}

/// An OWL expression reference set member: `owlExpression`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwlExpressionMember {
    /// The member.
    pub member: Member,
    /// The axiom in OWL functional syntax.
    pub owl_expression: String,
}

impl TryFrom<Member> for OwlExpressionMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        let owl_expression = member.string("owlExpression")?.to_owned();
        Ok(Self {
            member,
            owl_expression,
        })
    }
}

/// A reference set descriptor member: one attribute of a reference set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorMember {
    /// The member; `referenced_component_id` is the reference set described.
    pub member: Member,
    /// The concept describing the attribute.
    pub attribute_description: ConceptId,
    /// The attribute type concept.
    pub attribute_type: ConceptId,
    /// The zero-based order of the attribute; `0` is `referencedComponentId`.
    pub attribute_order: i64,
}

impl TryFrom<Member> for DescriptorMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        Ok(Self {
            attribute_description: member.concept("attributeDescription")?,
            attribute_type: member.concept("attributeType")?,
            attribute_order: member.integer("attributeOrder")?,
            member,
        })
    }
}

/// A description type reference set member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionTypeMember {
    /// The member; `referenced_component_id` is the description type concept.
    pub member: Member,
    /// The description format concept.
    pub description_format: ConceptId,
    /// The maximum length.
    pub description_length: i64,
}

impl TryFrom<Member> for DescriptionTypeMember {
    type Error = ViewError;

    fn try_from(member: Member) -> Result<Self, ViewError> {
        Ok(Self {
            description_format: member.concept("descriptionFormat")?,
            description_length: member.integer("descriptionLength")?,
            member,
        })
    }
}

/// What a reference set file holds, decided by the columns its header names.
///
/// A file name's summary is free-form: a derivative package writes its own
/// name into it, before the reference set type
/// (`der2_cRefset_ICNPLanguageSnapshot-en`) or after it
/// (`sct2_sRefset_OWLExpressionICNPFull`), so no substring of it is a reliable
/// signal. The additional columns are: the release file specification fixes
/// them per reference set pattern, and the header names them
/// (<https://docs.snomed.org/snomed-ct-specifications/snomed-ct-release-file-specification>).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefsetKind {
    /// A language reference set: `acceptabilityId`.
    Language,
    /// An OWL expression reference set: `owlExpression`.
    OwlExpression,
    /// The module dependency reference set: `sourceEffectiveTime` and
    /// `targetEffectiveTime`.
    ModuleDependency,
    /// Anything else: a simple, map, association, or attribute value set,
    /// which the build reads as content.
    Content,
}

impl RefsetKind {
    /// The kind the additional column names imply.
    #[must_use]
    pub fn of_columns(columns: &[String]) -> Self {
        let has = |name: &str| columns.iter().any(|column| column == name);
        if has("acceptabilityId") {
            Self::Language
        } else if has("owlExpression") {
            Self::OwlExpression
        } else if has("sourceEffectiveTime") && has("targetEffectiveTime") {
            Self::ModuleDependency
        } else {
            Self::Content
        }
    }
}

/// The kind of the reference set file at `path`, from its header alone.
///
/// # Errors
///
/// Returns [`Rf2Error`] when the file cannot be opened or its header does not
/// start with the six member columns.
pub fn kind(path: &Path) -> Result<RefsetKind, Rf2Error> {
    let file = std::fs::File::open(path).map_err(|source| Rf2Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = Rf2Reader::new_with(path, std::io::BufReader::new(file), |actual| {
        if actual.len() >= MEMBER_COLUMNS.len()
            && actual.iter().zip(MEMBER_COLUMNS).all(|(a, e)| a == e)
        {
            Ok(())
        } else {
            Err(Rf2Error::Header {
                path: path.to_path_buf(),
                expected: MEMBER_COLUMNS.iter().map(|c| (*c).to_owned()).collect(),
                actual: actual.to_vec(),
            })
        }
    })?;
    let additional: Vec<String> = reader
        .columns()
        .iter()
        .skip(MEMBER_COLUMNS.len())
        .cloned()
        .collect();
    Ok(RefsetKind::of_columns(&additional))
}

#[cfg(test)]
mod kind_tests {
    use super::RefsetKind;

    fn columns(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    /// The columns decide, whatever a derivative package called the file.
    #[test]
    fn the_additional_columns_name_the_kind() {
        assert_eq!(
            RefsetKind::of_columns(&columns(&["acceptabilityId"])),
            RefsetKind::Language
        );
        assert_eq!(
            RefsetKind::of_columns(&columns(&["owlExpression"])),
            RefsetKind::OwlExpression
        );
        assert_eq!(
            RefsetKind::of_columns(&columns(&["sourceEffectiveTime", "targetEffectiveTime"])),
            RefsetKind::ModuleDependency
        );
        assert_eq!(
            RefsetKind::of_columns(&columns(&["targetComponentId"])),
            RefsetKind::Content
        );
        assert_eq!(RefsetKind::of_columns(&[]), RefsetKind::Content);
    }
}
