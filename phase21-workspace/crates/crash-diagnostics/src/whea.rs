//! A bounded decoder for the WHEA Common Platform Error Record (CPER) that WHEA-Logger events carry
//! as their binary payload (P82-03A).
//!
//! The record is untrusted binary from the event log. Every length and offset is checked against
//! the bytes actually present, a record that does not validate decodes to nothing (never to a
//! guess), and an unknown section is `Other`, not a component. What the decoder can say is limited
//! to the record's own severity and the kinds of its sections; nothing here names a DIMM, a core
//! or a device.

const HEADER_BYTES: usize = 128;
const DESCRIPTOR_BYTES: usize = 72;
/// More sections than this is not a record this decoder will walk.
const MAX_SECTIONS: usize = 16;
const SIGNATURE: [u8; 4] = *b"CPER";
const SIGNATURE_END: u32 = 0xFFFF_FFFF;

/// `ERROR_SEVERITY` of the record header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WheaSeverity {
    Recoverable,
    Fatal,
    Corrected,
    Informational,
    /// A value the specification does not define. Not read as corrected or as fatal.
    Unknown,
}

/// The kind of one section, from its section-type GUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WheaSection {
    Memory,
    Processor,
    Pcie,
    /// Any other, or unrecognized, section type.
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WheaRecord {
    pub severity: WheaSeverity,
    pub sections: Vec<WheaSection>,
}

// Section-type GUIDs in their on-disk (mixed-endian) byte order.
// Processor generic {9876CCAD-47B4-4BDB-B65E-16F193C4F3DB}
const PROCESSOR: [u8; 16] = [
    0xAD, 0xCC, 0x76, 0x98, 0xB4, 0x47, 0xDB, 0x4B, 0xB6, 0x5E, 0x16, 0xF1, 0x93, 0xC4, 0xF3, 0xDB,
];
// Platform memory {A5BC1114-6F64-4EDE-B863-3E83ED7C83B1}
const MEMORY: [u8; 16] = [
    0x14, 0x11, 0xBC, 0xA5, 0x64, 0x6F, 0xDE, 0x4E, 0xB8, 0x63, 0x3E, 0x83, 0xED, 0x7C, 0x83, 0xB1,
];
// PCI Express {D995E954-BBC1-430F-AD91-B44DCB3C6F35}
const PCIE: [u8; 16] = [
    0x54, 0xE9, 0x95, 0xD9, 0xC1, 0xBB, 0x0F, 0x43, 0xAD, 0x91, 0xB4, 0x4D, 0xCB, 0x3C, 0x6F, 0x35,
];

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Decodes a CPER, or returns `None` when the bytes are not a valid one: a wrong signature, a
/// declared length the bytes do not hold, or a section descriptor that points outside the record.
pub(crate) fn decode_cper(bytes: &[u8]) -> Option<WheaRecord> {
    if bytes.len() < HEADER_BYTES || bytes.get(..4)? != SIGNATURE {
        return None;
    }
    if word(bytes, 6)? != SIGNATURE_END {
        return None;
    }
    let declared = usize::try_from(word(bytes, 20)?).ok()?;
    if declared < HEADER_BYTES || declared > bytes.len() {
        return None;
    }
    let record = &bytes[..declared];
    let count = usize::from(u16::from_le_bytes(record.get(10..12)?.try_into().ok()?));
    if count > MAX_SECTIONS {
        return None;
    }
    let severity = match word(record, 12)? {
        0 => WheaSeverity::Recoverable,
        1 => WheaSeverity::Fatal,
        2 => WheaSeverity::Corrected,
        3 => WheaSeverity::Informational,
        _ => WheaSeverity::Unknown,
    };
    let mut sections = Vec::with_capacity(count);
    for index in 0..count {
        let descriptor = HEADER_BYTES.checked_add(index.checked_mul(DESCRIPTOR_BYTES)?)?;
        let descriptor_end = descriptor.checked_add(DESCRIPTOR_BYTES)?;
        let descriptor_bytes = record.get(descriptor..descriptor_end)?;
        let offset = usize::try_from(word(descriptor_bytes, 0)?).ok()?;
        let length = usize::try_from(word(descriptor_bytes, 4)?).ok()?;
        // The section's own bytes must lie inside the record and after the descriptor table.
        if offset < HEADER_BYTES + count * DESCRIPTOR_BYTES
            || offset.checked_add(length)? > declared
        {
            return None;
        }
        sections.push(match descriptor_bytes.get(16..32)? {
            guid if guid == MEMORY => WheaSection::Memory,
            guid if guid == PROCESSOR => WheaSection::Processor,
            guid if guid == PCIE => WheaSection::Pcie,
            _ => WheaSection::Other,
        });
    }
    Some(WheaRecord { severity, sections })
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    /// A CPER with the given severity and one section per GUID (each section has 8 payload bytes).
    pub(crate) fn cper(severity: u32, guids: &[[u8; 16]]) -> Vec<u8> {
        let table = HEADER_BYTES + guids.len() * DESCRIPTOR_BYTES;
        let total = table + guids.len() * 8;
        let mut record = vec![0u8; total];
        record[..4].copy_from_slice(&SIGNATURE);
        record[4..6].copy_from_slice(&0x0100u16.to_le_bytes());
        record[6..10].copy_from_slice(&SIGNATURE_END.to_le_bytes());
        record[10..12].copy_from_slice(&(guids.len() as u16).to_le_bytes());
        record[12..16].copy_from_slice(&severity.to_le_bytes());
        record[20..24].copy_from_slice(&(total as u32).to_le_bytes());
        for (i, guid) in guids.iter().enumerate() {
            let d = HEADER_BYTES + i * DESCRIPTOR_BYTES;
            record[d..d + 4].copy_from_slice(&((table + i * 8) as u32).to_le_bytes());
            record[d + 4..d + 8].copy_from_slice(&8u32.to_le_bytes());
            record[d + 16..d + 32].copy_from_slice(guid);
        }
        record
    }

    pub(crate) const MEMORY_GUID: [u8; 16] = MEMORY;
    pub(crate) const PROCESSOR_GUID: [u8; 16] = PROCESSOR;
    pub(crate) const PCIE_GUID: [u8; 16] = PCIE;
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn the_severity_and_section_kinds_come_from_the_record_not_from_words() {
        let record = decode_cper(&cper(2, &[MEMORY_GUID])).expect("valid");
        assert_eq!(record.severity, WheaSeverity::Corrected);
        assert_eq!(record.sections, vec![WheaSection::Memory]);
        let fatal = decode_cper(&cper(1, &[PROCESSOR_GUID, PCIE_GUID, [7u8; 16]])).unwrap();
        assert_eq!(fatal.severity, WheaSeverity::Fatal);
        assert_eq!(
            fatal.sections,
            vec![
                WheaSection::Processor,
                WheaSection::Pcie,
                WheaSection::Other
            ]
        );
        assert_eq!(
            decode_cper(&cper(9, &[])).unwrap().severity,
            WheaSeverity::Unknown
        );
    }

    #[test]
    fn a_record_that_does_not_validate_decodes_to_nothing() {
        let good = cper(2, &[MEMORY_GUID]);
        assert!(decode_cper(&[]).is_none());
        assert!(
            decode_cper(&good[..100]).is_none(),
            "shorter than the header"
        );
        let mut bad_sig = good.clone();
        bad_sig[0] = b'X';
        assert!(decode_cper(&bad_sig).is_none());
        let mut bad_end = good.clone();
        bad_end[6] = 0;
        assert!(decode_cper(&bad_end).is_none());
        assert!(
            decode_cper(&good[..good.len() - 1]).is_none(),
            "declared length larger than the bytes"
        );
    }

    #[test]
    fn hostile_counts_and_offsets_never_panic_and_never_guess() {
        let mut too_many = cper(2, &[MEMORY_GUID]);
        too_many[10..12].copy_from_slice(&500u16.to_le_bytes());
        assert!(decode_cper(&too_many).is_none());
        let d = HEADER_BYTES;
        let mut out_of_range = cper(2, &[MEMORY_GUID]);
        out_of_range[d..d + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            decode_cper(&out_of_range).is_none(),
            "an offset outside the record is a malformed record"
        );
        let mut overflow = cper(2, &[MEMORY_GUID]);
        overflow[d + 4..d + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_cper(&overflow).is_none());
        let mut into_table = cper(2, &[MEMORY_GUID]);
        into_table[d..d + 4].copy_from_slice(&0u32.to_le_bytes());
        assert!(
            decode_cper(&into_table).is_none(),
            "a section may not start inside the header or descriptor table"
        );
    }
}
