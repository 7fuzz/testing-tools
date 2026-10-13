pub struct Region {
    pub province: &'static str,
    pub city: &'static str,
    /// The 4-digit province + city part of the wilayah code; the district index adds two more.
    pub code: &'static str,
    pub districts: &'static [(&'static str, &'static [&'static str])],
}

pub const REGIONS: &[Region] = &[
    Region {
        province: "JAWA BARAT",
        city: "KOTA BANDUNG",
        code: "3273",
        districts: &[
            ("CIDADAP", &["HEGARMANAH", "CIUMBULEUIT", "LEDENG"]),
            ("COBLONG", &["DAGO", "LEBAKSILIWANGI", "SEKELOA"]),
            ("SUKAJADI", &["PASTEUR", "CIPEDES", "SUKAWARNA"]),
        ],
    },
    Region {
        province: "JAWA BARAT",
        city: "KABUPATEN BOGOR",
        code: "3201",
        districts: &[("CIBINONG", &["PAKANSARI", "NANGGEWER", "TENGAH"]), ("CITEUREUP", &["PUSPANEGARA", "TARIKOLOT"])],
    },
    Region {
        province: "DKI JAKARTA",
        city: "JAKARTA SELATAN",
        code: "3171",
        districts: &[
            ("KEBAYORAN BARU", &["SENAYAN", "MELAWAI", "GANDARIA UTARA"]),
            ("TEBET", &["TEBET BARAT", "MANGGARAI", "KEBON BARU"]),
        ],
    },
    Region {
        province: "DKI JAKARTA",
        city: "JAKARTA TIMUR",
        code: "3172",
        districts: &[
            ("CAKUNG", &["PULO GEBANG", "JATINEGARA", "PENGGILINGAN"]),
            ("DUREN SAWIT", &["PONDOK BAMBU", "KLENDER"]),
        ],
    },
    Region {
        province: "JAWA TENGAH",
        city: "KOTA SEMARANG",
        code: "3374",
        districts: &[
            ("BANYUMANIK", &["SRONDOL WETAN", "PEDALANGAN"]),
            ("TEMBALANG", &["BULUSAN", "METESEH", "SENDANGMULYO"]),
        ],
    },
    Region {
        province: "DI YOGYAKARTA",
        city: "KOTA YOGYAKARTA",
        code: "3471",
        districts: &[
            ("GONDOKUSUMAN", &["BACIRO", "DEMANGAN", "TERBAN"]),
            ("UMBULHARJO", &["WARUNGBOTO", "MUJA MUJU", "PANDEYAN"]),
        ],
    },
    Region {
        province: "JAWA TIMUR",
        city: "KOTA SURABAYA",
        code: "3578",
        districts: &[("GUBENG", &["AIRLANGGA", "MOJO", "KERTAJAYA"]), ("WONOKROMO", &["DARMO", "NGAGEL", "JAGIR"])],
    },
    Region {
        province: "BANTEN",
        city: "KOTA TANGERANG",
        code: "3671",
        districts: &[("CILEDUG", &["SUDIMARA BARAT", "PARUNG SERAB"]), ("KARAWACI", &["BOJONG JAYA", "NUSA JAYA"])],
    },
    Region {
        province: "BALI",
        city: "KOTA DENPASAR",
        code: "5171",
        districts: &[
            ("DENPASAR SELATAN", &["SANUR", "SESETAN", "PANJER"]),
            ("DENPASAR BARAT", &["PADANGSAMBIAN", "DAUH PURI"]),
        ],
    },
    Region {
        province: "SUMATERA UTARA",
        city: "KOTA MEDAN",
        code: "1275",
        districts: &[("MEDAN BARU", &["PADANG BULAN", "PETISAH HULU"]), ("MEDAN JOHOR", &["GEDUNG JOHOR", "KWALA BEKALA"])],
    },
    Region {
        province: "SULAWESI SELATAN",
        city: "KOTA MAKASSAR",
        code: "7371",
        districts: &[("PANAKKUKANG", &["MASALE", "KARAMPUANG"]), ("TAMALATE", &["MANGASA", "JONGAYA"])],
    },
];

pub const BIRTH_PLACES: &[&str] = &[
    "BANDUNG", "JAKARTA", "SURABAYA", "SEMARANG", "YOGYAKARTA", "MEDAN", "MAKASSAR", "DENPASAR", "BOGOR", "TANGERANG", "GARUT",
    "SOLO", "MALANG", "PADANG", "PALEMBANG", "CIREBON", "TASIKMALAYA", "KEDIRI", "PEKALONGAN", "BALIKPAPAN",
];

pub const FIRST_NAMES_MALE: &[&str] = &[
    "BUDI", "AGUS", "ANDI", "RIZKY", "DIMAS", "FAJAR", "HENDRA", "IRFAN", "JOKO", "BAYU", "YUSUF", "ARIF", "DEDI", "EKO",
    "GILANG", "RAHMAT", "WAHYU", "TAUFIK", "ADITYA", "REZA",
];
pub const FIRST_NAMES_FEMALE: &[&str] = &[
    "SITI", "DEWI", "RINA", "PUTRI", "AYU", "INDAH", "NUR", "FITRI", "WULAN", "ANISA", "MAYA", "RATNA", "YULIA", "DIAN", "SRI",
    "NOVITA", "MELATI", "INTAN", "SARI", "NADIA",
];
pub const LAST_NAMES_MALE: &[&str] = &["PRATAMA", "SAPUTRA", "RAMADHAN", "FIRMANSYAH", "MAULANA", "HIDAYAT", "KURNIAWAN"];
pub const LAST_NAMES_FEMALE: &[&str] = &["LESTARI", "SAPUTRI", "RAHMAWATI", "PERMATASARI", "WULANDARI", "RAHAYU", "KARTIKA"];
pub const LAST_NAMES_ANY: &[&str] = &[
    "WIJAYA", "SANTOSO", "NUGROHO", "SETIAWAN", "HALIM", "SIREGAR", "NASUTION", "LUBIS", "HARAHAP", "GINTING", "TANJUNG",
    "PERMANA", "SUSANTO", "SIMANJUNTAK",
];

pub const STREETS: &[&str] = &[
    "MERDEKA", "SUDIRMAN", "MAWAR", "MELATI", "ANGGREK", "KENANGA", "PAHLAWAN", "DIPONEGORO", "GATOT SUBROTO", "AHMAD YANI",
    "CEMPAKA", "KUCING MANIS", "RAYA BARAT", "H. SALEH", "PELITA", "SUKAMAJU", "NUSANTARA", "TERATAI",
];
pub const HOUSING: &[&str] = &["GRIYA ASRI", "PERMATA HIJAU", "BUMI INDAH", "TAMAN SARI", "PURI KENCANA"];

pub const RELIGIONS: &[&str] = &["ISLAM", "KRISTEN", "KATHOLIK", "HINDU", "BUDHA", "KONGHUCU"];
pub const MARITAL: &[&str] = &["BELUM KAWIN", "KAWIN", "CERAI HIDUP", "CERAI MATI"];
pub const BLOOD: &[&str] = &["-", "O", "A", "B", "AB"];
pub const JOBS: &[&str] = &[
    "KARYAWAN SWASTA", "WIRASWASTA", "PEGAWAI NEGERI SIPIL", "PEDAGANG", "GURU", "PETANI/PEKEBUN", "BURUH HARIAN LEPAS",
    "KARYAWAN BUMN", "DOKTER", "PERAWAT", "SOPIR", "NELAYAN/PERIKANAN",
];
pub const STUDENT: &str = "PELAJAR/MAHASISWA";
pub const RETIRED: &str = "PENSIUNAN";
pub const HOMEMAKER: &str = "MENGURUS RUMAH TANGGA";
