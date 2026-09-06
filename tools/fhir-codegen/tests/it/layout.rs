//! The width of the emitted choice enums.
//!
//! A Rust enum is as large as its largest variant, so one wide variant sets the
//! size every value of that enum pays. The emitter boxes every choice variant
//! holding a complex type, which leaves each enum as wide as the widest
//! primitive it admits. Without that, `Parameters.value[x]` was 4512 bytes,
//! because it admits `Dosage` and a `Dosage` carries a whole `Timing`, and every
//! `valueString` of an answer cost the same as a dosage schedule (#378).

use std::mem::size_of;

/// The bound a choice enum stays under: two cache lines, which the widest
/// primitive (a value, an id, and an extension list) fits inside with room.
const NARROW: usize = 128;

#[test]
fn a_choice_enum_is_no_wider_than_the_primitives_it_admits() {
    let widths = [
        (
            "r4 value[x]",
            size_of::<fhir_types::r4::parameters::ParametersParameterValue>(),
        ),
        (
            "r4b value[x]",
            size_of::<fhir_types::r4b::parameters::ParametersParameterValue>(),
        ),
        (
            "r5 value[x]",
            size_of::<fhir_types::r5::parameters::ParametersParameterValue>(),
        ),
        (
            "r6 value[x]",
            size_of::<fhir_types::r6::parameters::ParametersParameterValue>(),
        ),
        (
            "r4 Extension.value[x]",
            size_of::<fhir_types::r4::extension::ExtensionValue>(),
        ),
        (
            "r4b Extension.value[x]",
            size_of::<fhir_types::r4b::extension::ExtensionValue>(),
        ),
        (
            "r5 Extension.value[x]",
            size_of::<fhir_types::r5::extension::ExtensionValue>(),
        ),
        (
            "r6 Extension.value[x]",
            size_of::<fhir_types::r6::extension::ExtensionValue>(),
        ),
    ];
    for (name, width) in widths {
        assert!(
            width <= NARROW,
            "{name} is {width} bytes, over the {NARROW} the emitter's boxing holds it to: \
             a complex variant is inline again, so every value of the enum pays for it"
        );
    }
}

#[test]
fn a_parameter_costs_its_own_value_and_not_the_widest_one() {
    // Every output of an answer is one of these, so its width multiplies by the
    // number of parameters a `$lookup` writes (hundreds, for RxNorm).
    let widths = [
        (
            "r4",
            size_of::<fhir_types::r4::parameters::ParametersParameter>(),
        ),
        (
            "r4b",
            size_of::<fhir_types::r4b::parameters::ParametersParameter>(),
        ),
        (
            "r5",
            size_of::<fhir_types::r5::parameters::ParametersParameter>(),
        ),
        (
            "r6",
            size_of::<fhir_types::r6::parameters::ParametersParameter>(),
        ),
    ];
    for (version, width) in widths {
        assert!(
            width <= 512,
            "{version} ParametersParameter is {width} bytes, so an answer of hundreds of \
             parameters memmoves that many times more than its content"
        );
    }
}
