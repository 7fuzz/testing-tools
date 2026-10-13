pub const NAME_FIRST: &[&str] = &["WARUNG", "TOKO", "KEDAI", "KOPI", "RM", "BAKSO", "MIE", "SATE", "DAPUR", "ANGKRINGAN"];
pub const NAME_SECOND: &[&str] =
    &["KUCING", "OREN", "SEJAHTERA", "BAROKAH", "MAKMUR", "JAYA", "NUSANTARA", "BAHAGIA", "SENJA", "PELANGI"];

pub const CITIES: &[(&str, &str)] = &[
    ("BANDUNG", "40115"),
    ("JAKARTA", "10110"),
    ("SURABAYA", "60111"),
    ("YOGYAKARTA", "55111"),
    ("SEMARANG", "50132"),
    ("MEDAN", "20111"),
    ("BOGOR", "16111"),
    ("DENPASAR", "80111"),
];

pub const MCCS: &[(&str, &str)] = &[
    ("5814", "Fast food"),
    ("5812", "Restaurant"),
    ("5411", "Grocery / supermarket"),
    ("5499", "Other food store"),
    ("5999", "Other retail"),
    ("7230", "Salon / barbershop"),
];

/// UMI micro, UKE small, UME medium, UBE large, URE regular.
pub const CRITERIA: &[(&str, &str)] =
    &[("UMI", "Micro"), ("UKE", "Small"), ("UME", "Medium"), ("UBE", "Large"), ("URE", "Regular")];

pub const RANDOM_AMOUNTS: &[u64] = &[5_000, 10_000, 15_000, 25_000, 50_000];
