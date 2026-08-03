//! Which byte-shuffle instruction set this binary is allowed to use, decided once
//! from `cfg` and a runtime probe — so [`crate::shuffle`]'s composition kernel and
//! [`crate::skip`]'s nibble classifier can never independently disagree about the
//! target's silicon, and a third instruction set, when one arrives, is taught the
//! detection ladder in one place instead of two.
//!
//! The unsafe intrinsics themselves live one level down, one file per instruction
//! set ([`neon`], [`ssse3`]) rather than interleaved with the algorithms that call
//! them: an audit of "what does this crate execute on x86_64" reads one file, not
//! two half-files split across [`crate::shuffle`] and [`crate::skip`].

#[cfg(target_arch = "aarch64")]
pub(crate) mod neon;
#[cfg(target_arch = "x86_64")]
pub(crate) mod ssse3;

/// Bytes per vector step for every kernel in this module: one 128-bit register,
/// which is what both `vqtbl1q_u8`/`vld1q_u8` and `pshufb`/`_mm_loadu_si128` index.
pub(crate) const STEP: usize = 16;

/// Which byte-shuffle instruction set backs the sieve on this target.
///
/// Reported rather than assumed, for two reasons that are both about not lying.
/// A differential test that compares the vector kernel against the scalar
/// reference proves nothing if dispatch already chose `Scalar` — it would be
/// comparing a function to itself and passing vacuously — so a test has to be able
/// to ask. And [`crate::price`] was measured with a byte shuffle; a target that has
/// none runs a materially slower kernel, which the gate must know rather than
/// discover in production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kernel {
    /// `vqtbl1q_u8` — baseline on every `aarch64`.
    Neon,
    /// `pshufb` — probed at runtime on `x86_64`.
    Ssse3,
    /// No byte shuffle on this target: the reference path is the shipping path.
    Scalar,
}

impl Kernel {
    /// Does this kernel do 16 lanes at a time? The economics in [`crate::price`]
    /// assume it does.
    #[must_use]
    pub const fn is_vector(self) -> bool {
        !matches!(self, Self::Scalar)
    }
}

/// Which kernel every dispatch point in this crate will actually run here. Decided
/// by the same `cfg` and the same runtime probe every call site matches against,
/// so what runs and what the crate reports are always the same decision.
#[must_use]
pub fn kernel() -> Kernel {
    #[cfg(target_arch = "aarch64")]
    return Kernel::Neon;

    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("ssse3") {
        Kernel::Ssse3
    } else {
        Kernel::Scalar
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    Kernel::Scalar
}
