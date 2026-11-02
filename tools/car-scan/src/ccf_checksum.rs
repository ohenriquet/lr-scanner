//! Checksum do CCF (CHECKSUM_CCF_EUCD).
//!
//! Ainda sendo reconstruído da `VbfConversion.dll` (CCCFConvert::UpdateChecksum).
//! Até lá, `fix_checksums` recusa a operação, para que nenhuma gravação com
//! checksum inválido chegue ao módulo.

/// Recalcula e grava os checksums na imagem do CCF, no lugar.
pub fn fix_checksums(_image: &mut [u8]) -> anyhow::Result<()> {
    anyhow::bail!(
        "checksum do CCF ainda não reconstruído; a gravação de alteração está bloqueada \
         por segurança. Use ccf-writeback (sem alteração) para validar a tubulação."
    )
}
