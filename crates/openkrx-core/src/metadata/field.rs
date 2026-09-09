//! Schema-fixed element identities.
//!
//! Every variant names an element that `KER_META_V0_9` itself defines
//! (`docs/profile.md` rules M1 to M8). A [`MetadataField`] is therefore part of
//! the grammar, never document content: it can be reported without disclosing
//! anything an attacker put in the file.

/// One element the metadata grammar defines.
///
/// The enum is `#[non_exhaustive]`: a later profile revision may add elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MetadataField {
    /// Root element `KULDEMENY` (M1).
    Kuldemeny,
    /// `FEJRESZ` (M2).
    Fejresz,
    /// `ERKEZTETES` (M2).
    Erkeztetes,
    /// `BONTASOK` (M2).
    Bontasok,
    /// `EXPEDIALASOK` (M2).
    Expedialasok,
    /// `EXPEDIALAS` (M7).
    Expedialas,
    /// `TERTIVEVENY` (M2).
    Tertiveveny,
    /// `KRX_VERZIOSZAM` (M3).
    KrxVerzioszam,
    /// `FORRASRENDSZER_AZONOSITO` (M3, M4).
    ForrasrendszerAzonosito,
    /// `KULDEMENY_AZONOSITO` (M3).
    KuldemenyAzonosito,
    /// `KULDEMENY_LETREHOZASANAK_IDEJE` (M3).
    KuldemenyLetrehozasanakIdeje,
    /// `KULDEMENY_TIPUS` (M3, M4).
    KuldemenyTipus,
    /// `TESZT` (M3).
    Teszt,
    /// `VONALKOD` (M3, optional).
    Vonalkod,
    /// `KULDEMENY_HIVATKOZASI_AZONOSITO` (M3, optional).
    KuldemenyHivatkozasiAzonosito,
    /// `HIBAKOD` (M3, optional).
    Hibakod,
    /// `KULDEMENY_MEGJEGYZES` (M3, optional).
    KuldemenyMegjegyzes,
    /// `MELLEKLETEK` (M7).
    Mellekletek,
    /// `MELLEKLETEK_SZAMA` (M7).
    MellekletekSzama,
    /// `MELLEKLET` (M5).
    Melleklet,
    /// `MELLEKLET_LEIRASA` (M5).
    MellekletLeirasa,
    /// `CSATOLMANY_SZAMA` (M5).
    CsatolmanySzama,
    /// `FAJL_NEV` (M5).
    FajlNev,
    /// `MERET` (M5, M6).
    Meret,
    /// `ELHELYEZKEDES` (M5, M6).
    Elhelyezkedes,
    /// `MENNYISEG` (M5, optional).
    Mennyiseg,
    /// `MENNYISEGI_EGYSEG` (M5, optional).
    MennyisegiEgyseg,
    /// `KEZELESI_UTASITASOK`, namespace-unqualified (M8).
    KezelesiUtasitasok,
}

impl MetadataField {
    /// The element's local name exactly as the schema spells it.
    #[must_use]
    pub const fn local_name(self) -> &'static str {
        match self {
            Self::Kuldemeny => "KULDEMENY",
            Self::Fejresz => "FEJRESZ",
            Self::Erkeztetes => "ERKEZTETES",
            Self::Bontasok => "BONTASOK",
            Self::Expedialasok => "EXPEDIALASOK",
            Self::Expedialas => "EXPEDIALAS",
            Self::Tertiveveny => "TERTIVEVENY",
            Self::KrxVerzioszam => "KRX_VERZIOSZAM",
            Self::ForrasrendszerAzonosito => "FORRASRENDSZER_AZONOSITO",
            Self::KuldemenyAzonosito => "KULDEMENY_AZONOSITO",
            Self::KuldemenyLetrehozasanakIdeje => "KULDEMENY_LETREHOZASANAK_IDEJE",
            Self::KuldemenyTipus => "KULDEMENY_TIPUS",
            Self::Teszt => "TESZT",
            Self::Vonalkod => "VONALKOD",
            Self::KuldemenyHivatkozasiAzonosito => "KULDEMENY_HIVATKOZASI_AZONOSITO",
            Self::Hibakod => "HIBAKOD",
            Self::KuldemenyMegjegyzes => "KULDEMENY_MEGJEGYZES",
            Self::Mellekletek => "MELLEKLETEK",
            Self::MellekletekSzama => "MELLEKLETEK_SZAMA",
            Self::Melleklet => "MELLEKLET",
            Self::MellekletLeirasa => "MELLEKLET_LEIRASA",
            Self::CsatolmanySzama => "CSATOLMANY_SZAMA",
            Self::FajlNev => "FAJL_NEV",
            Self::Meret => "MERET",
            Self::Elhelyezkedes => "ELHELYEZKEDES",
            Self::Mennyiseg => "MENNYISEG",
            Self::MennyisegiEgyseg => "MENNYISEGI_EGYSEG",
            Self::KezelesiUtasitasok => "KEZELESI_UTASITASOK",
        }
    }

    /// The field a local name denotes, when the grammar defines one.
    ///
    /// Matching is byte-exact: the schema fixes these spellings, and accepting
    /// a case variant would resolve an ambiguity the sources do not settle.
    #[must_use]
    pub fn from_local_name(name: &str) -> Option<Self> {
        const ALL: [MetadataField; 28] = [
            MetadataField::Kuldemeny,
            MetadataField::Fejresz,
            MetadataField::Erkeztetes,
            MetadataField::Bontasok,
            MetadataField::Expedialasok,
            MetadataField::Expedialas,
            MetadataField::Tertiveveny,
            MetadataField::KrxVerzioszam,
            MetadataField::ForrasrendszerAzonosito,
            MetadataField::KuldemenyAzonosito,
            MetadataField::KuldemenyLetrehozasanakIdeje,
            MetadataField::KuldemenyTipus,
            MetadataField::Teszt,
            MetadataField::Vonalkod,
            MetadataField::KuldemenyHivatkozasiAzonosito,
            MetadataField::Hibakod,
            MetadataField::KuldemenyMegjegyzes,
            MetadataField::Mellekletek,
            MetadataField::MellekletekSzama,
            MetadataField::Melleklet,
            MetadataField::MellekletLeirasa,
            MetadataField::CsatolmanySzama,
            MetadataField::FajlNev,
            MetadataField::Meret,
            MetadataField::Elhelyezkedes,
            MetadataField::Mennyiseg,
            MetadataField::MennyisegiEgyseg,
            MetadataField::KezelesiUtasitasok,
        ];
        ALL.into_iter().find(|field| field.local_name() == name)
    }
}
