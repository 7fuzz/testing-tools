use crate::generator::ErasedGenerator;

/// Every image generator, for tools that work with any of them (VCam layers, the CLI).
pub const BUILTIN: &[ErasedGenerator] = &[
    ErasedGenerator::of::<crate::qris::Qris>(),
    ErasedGenerator::of::<crate::ktp::Ktp>(),
    ErasedGenerator::of::<crate::ktp::KtpSelfie>(),
];

pub fn find(id: &str) -> Option<&'static ErasedGenerator> {
    BUILTIN.iter().find(|g| g.id == id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = super::BUILTIN.iter().map(|g| g.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), super::BUILTIN.len());
    }
}
