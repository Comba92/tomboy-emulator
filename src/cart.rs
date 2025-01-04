use core::{cmp, hash, str};

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct CartHeader {
    pub cart_type: &'static str,
    pub mapper_code: u8,
    title: String,
    licensee: &'static str,
    licensee_new: &'static str,
    region: Region,
    pub cgb_mode: CgbMode,
    pub sgb_support: bool,
    pub rom_banks: usize,
    pub rom_size: usize,
    pub ram_banks: usize,
    pub ram_size: usize,
    pub has_battery: bool,
    version: u8,
    checksum: u8,
}

const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

#[derive(Debug, Clone)]
pub enum CgbMode { Monochrome, CgbEnhanced, ColorOnly }
#[derive(Debug, Clone)]
pub enum Region { Japan, Overseas } 

fn parse_info<Info: cmp::Eq + hash::Hash, Parsed: Copy>(
    code: Info, 
    // map: &HashMap<Info, Parsed>,
    map: &[(Info, Parsed)],
    err: &'static str
) -> Result<Parsed, &'static str> {
    map.iter().find(|i| i.0 == code)
    .map(|o| o.1)
    .ok_or(err)
}

pub fn is_gb_rom(bytes: &[u8]) -> bool {
    if bytes.len() < 0x104 + (0x14F - 0x104) {
        return false;
    }

    bytes[0x104..=0x133] == NINTENDO_LOGO
}

impl CartHeader {
    pub fn new(bytes: &[u8]) -> Result<Self, &str> {
        if bytes.len() < 0x104 + (0x14F - 0x104) {
            return Err("Rom file is too small")
        }

        if bytes[0x104..=0x133] != NINTENDO_LOGO {
            return Err("Nintendo logo not found");
        }

        let title = str
            ::from_utf8(&bytes[0x134..0x143])
            .map(|s| String::from(s))
            .map_err(|_| "Invalid title")?
            .chars()
            .filter(|c| !c.is_control())
            .collect();

        let cgb_mode = match bytes[0x143] {
            0x80 => CgbMode::CgbEnhanced,
            0xC0 => CgbMode::ColorOnly,
            _ => CgbMode::Monochrome,
        };

        let sgb_support = bytes[0x146] != 0;

        let mapper_code = bytes[0x147];
        let cart_type = 
            parse_info(mapper_code, &CART_TYPE_MAP, "Invalid cart type")?;
        let has_battery = cart_type.contains("BATTERY");

        let rom_size_id = bytes[0x148];
        let rom_banks = 
            parse_info(rom_size_id, &ROM_SIZE_MAP, "Invalid ROM size")?;
        let rom_size = 16*1024*rom_banks;

        let ram_size_id = bytes[0x149];
        let ram_banks = 
            parse_info(ram_size_id, &RAM_SIZE_MAP, "Invalid RAM size")?;
        let ram_size = 8*1024*ram_banks;

        let region = match bytes[0x14a] != 0 {
            false => Region::Japan,
            true  => Region::Overseas,
        };

        let licensee_id = bytes[0x14b];
        let licensee = 
            parse_info(licensee_id, &LICENSEE_MAP, "Invalid old licensee")?;

        let licensee_new = if licensee_id == 0x33 {
            let licensee_new_str = str
                ::from_utf8(&bytes[0x144..=0x145])
                .unwrap_or("00");
            let licensee_new = 
                parse_info(licensee_new_str, &NEW_LICESEE_MAP, "Invalid new licensee")
                .unwrap_or("None");
            licensee_new
        } else {
            NEW_LICESEE_MAP.iter().find(|i| i.0 == "00").unwrap().1
        };

        let version = bytes[0x14c];
        let checksum = bytes[0x14d];

        let mut check = 0u8;
        for addr in 0x0134..=0x14C {
            check = check.wrapping_sub(bytes[addr]).wrapping_sub(1);
        }

        if check != checksum {
            return Err("Invalid checksum");
        }

        Ok(Self {
            title,
            mapper_code,
            cgb_mode,
            sgb_support,
            cart_type,
            licensee,
            licensee_new,
            region,
            rom_banks,
            ram_banks,
            rom_size,
            ram_size,
            has_battery,
            version,
            checksum,
        })
    }
}

#[cfg(test)]
mod cart_tests {
    use super::CartHeader;

    #[test]
    fn read_rom() {
        let rom = std::fs::read_dir("roms/").unwrap();
        for file in rom {
            let file = std::fs::read(file.unwrap().path()).unwrap();
            match CartHeader::new(&file) {
                Ok(cart) => println!("{:?}", cart),
                Err(e) => println!("{e}"),
            }
        }
    }
}

const NEW_LICESEE_MAP: [(&str, &str); 64] = [
    ("00", "None"),
    ("01", "Nintendo Research & Development 1"),
    ("08", "Capcom"),
    ("13", "EA (Electronic Arts)"),
    ("18", "Hudson Soft"),
    ("19", "B-AI"),
    ("20", "KSS"),
    ("22", "Planning Office WADA"),
    ("24", "PCM Complete"),
    ("25", "San-X"),
    ("28", "Kemco"),
    ("29", "SETA Corporation"),
    ("30", "Viacom"),
    ("31", "Nintendo"),
    ("32", "Bandai"),
    ("33", "Ocean Software/Acclaim Entertainment"),
    ("34", "Konami"),
    ("35", "HectorSoft"),
    ("37", "Taito"),
    ("38", "Hudson Soft"),
    ("39", "Banpresto"),
    ("41", "Ubi Soft1"),
    ("42", "Atlus"),
    ("44", "Malibu Interactive"),
    ("46", "Angel"),
    ("47", "Bullet-Proof Software2"),
    ("49", "Irem"),
    ("50", "Absolute"),
    ("51", "Acclaim Entertainment"),
    ("52", "Activision"),
    ("53", "Sammy USA Corporation"),
    ("54", "Konami"),
    ("55", "Hi Tech Expressions"),
    ("56", "LJN"),
    ("57", "Matchbox"),
    ("58", "Mattel"),
    ("59", "Milton Bradley Company"),
    ("60", "Titus Interactive"),
    ("61", "Virgin Games Ltd.3"),
    ("64", "Lucasfilm Games4"),
    ("67", "Ocean Software"),
    ("69", "EA (Electronic Arts)"),
    ("70", "Infogrames5"),
    ("71", "Interplay Entertainment"),
    ("72", "Broderbund"),
    ("73", "Sculptured Software6"),
    ("75", "The Sales Curve Limited7"),
    ("78", "THQ"),
    ("79", "Accolade"),
    ("80", "Misawa Entertainment"),
    ("83", "lozc"),
    ("86", "Tokuma Shoten"),
    ("87", "Tsukuda Original"),
    ("91", "Chunsoft Co.8"),
    ("92", "Video System"),
    ("93", "Ocean Software/Acclaim Entertainment"),
    ("95", "Varie"),
    ("96", "Yonezawa/s’pal"),
    ("97", "Kaneko"),
    ("99", "Pack-In-Video"),
    ("9H", "Bottom Up"),
    ("A4", "Konami (Yu-Gi-Oh!)"),
    ("BL", "MTO"),
    ("DK", "Kodansha"),
];

const CART_TYPE_MAP: [(u8, &str); 28] = [
    (0x00, "ROM ONLY"),
    (0x01, "MBC1"),
    (0x02, "MBC1+RAM"),
    (0x03, "MBC1+RAM+BATTERY"),
    (0x05, "MBC2"),
    (0x06, "MBC2+BATTERY"),
    (0x08, "ROM+RAM"),
    (0x09, "ROM+RAM+BATTERY"),
    (0x0B, "MMM01"),
    (0x0C, "MMM01+RAM"),
    (0x0D, "MMM01+RAM+BATTERY"),
    (0x0F, "MBC3+TIMER+BATTERY"),
    (0x10, "MBC3+TIMER+RAM+BATTERY"),
    (0x11, "MBC3"),
    (0x12, "MBC3+RAM"),
    (0x13, "MBC3+RAM+BATTERY"),
    (0x19, "MBC5"),
    (0x1A, "MBC5+RAM"),
    (0x1B, "MBC5+RAM+BATTERY"),
    (0x1C, "MBC5+RUMBLE"),
    (0x1D, "MBC5+RUMBLE+RAM"),
    (0x1E, "MBC5+RUMBLE+RAM+BATTERY"),
    (0x20, "MBC6"),
    (0x22, "MBC7+SENSOR+RUMBLE+RAM+BATTERY"),
    (0xFC, "POCKET CAMERA"),
    (0xFD, "BANDAI TAMA5"),
    (0xFE, "HuC3"),
    (0xFF, "HuC1+RAM+BATTERY"),
];

const ROM_SIZE_MAP: [(u8, usize); 12] = [
    (0x00, 2),
    (0x01, 4),
    (0x02, 8),
    (0x03, 16),
    (0x04, 32),
    (0x05, 64),
    (0x06, 128),
    (0x07, 256),
    (0x08, 512),
    (0x52, 72),
    (0x53, 80),
    (0x54, 96),
];

const RAM_SIZE_MAP: [(u8, usize); 6] = [
    (0x00, 0),
    (0x01, 0),
    (0x02, 1),
    (0x03, 4),
    (0x04, 16),
    (0x05, 8),
];

const LICENSEE_MAP: [(u8, &str); 147] = [
    (0x00,	"None"),
    (0x01,	"Nintendo"),
    (0x08,	"Capcom"),
    (0x09,	"HOT-B"),
    (0x0A,	"Jaleco"),
    (0x0B,	"Coconuts Japan"),
    (0x0C,	"Elite Systems"),
    (0x13,	"EA (Electronic Arts)"),
    (0x18,	"Hudson Soft"),
    (0x19,	"ITC Entertainment"),
    (0x1A,	"Yanoman"),
    (0x1D,	"Japan Clary"),
    (0x1F,	"Virgin Games Ltd.3"),
    (0x24,	"PCM Complete"),
    (0x25,	"San-X"),
    (0x28,	"Kemco"),
    (0x29,	"SETA Corporation"),
    (0x30,	"Infogrames5"),
    (0x31,	"Nintendo"),
    (0x32,	"Bandai"),
    (0x33,	"Indicates that the New licensee code should be used instead."),
    (0x34,	"Konami"),
    (0x35,	"HectorSoft"),
    (0x38,	"Capcom"),
    (0x39,	"Banpresto"),
    (0x3C,	"Entertainment Interactive (stub)"),
    (0x3E,	"Gremlin"),
    (0x41,	"Ubi Soft1"),
    (0x42,	"Atlus"),
    (0x44,	"Malibu Interactive"),
    (0x46,	"Angel"),
    (0x47,	"Spectrum HoloByte"),
    (0x49,	"Irem"),
    (0x4A,	"Virgin Games Ltd.3"),
    (0x4D,	"Malibu Interactive"),
    (0x4F,	"U.S. Gold"),
    (0x50,	"Absolute"),
    (0x51,	"Acclaim Entertainment"),
    (0x52,	"Activision"),
    (0x53,	"Sammy USA Corporation"),
    (0x54,	"GameTek"),
    (0x55,	"Park Place13"),
    (0x56,	"LJN"),
    (0x57,	"Matchbox"),
    (0x59,	"Milton Bradley Company"),
    (0x5A,	"Mindscape"),
    (0x5B,	"Romstar"),
    (0x5C,	"Naxat Soft14"),
    (0x5D,	"Tradewest"),
    (0x60,	"Titus Interactive"),
    (0x61,	"Virgin Games Ltd.3"),
    (0x67,	"Ocean Software"),
    (0x69,	"EA (Electronic Arts)"),
    (0x6E,	"Elite Systems"),
    (0x6F,	"Electro Brain"),
    (0x70,	"Infogrames5"),
    (0x71,	"Interplay Entertainment"),
    (0x72,	"Broderbund"),
    (0x73,	"Sculptured Software6"),
    (0x75,	"The Sales Curve Limited7"),
    (0x78,	"THQ"),
    (0x79,	"Accolade15"),
    (0x7A,	"Triffix Entertainment"),
    (0x7C,	"MicroProse"),
    (0x7F,	"Kemco"),
    (0x80,	"Misawa Entertainment"),
    (0x83,	"LOZC G."),
    (0x86,	"Tokuma Shoten"),
    (0x8B,	"Bullet-Proof Software2"),
    (0x8C,	"Vic Tokai Corp.16"),
    (0x8E,	"Ape Inc.17"),
    (0x8F,	"I’Max18"),
    (0x91,	"Chunsoft Co.8"),
    (0x92,	"Video System"),
    (0x93,	"Tsubaraya Productions"),
    (0x95,	"Varie"),
    (0x96,	"Yonezawa19/S’Pal"),
    (0x97,	"Kemco"),
    (0x99,	"Arc"),
    (0x9A,	"Nihon Bussan"),
    (0x9B,	"Tecmo"),
    (0x9C,	"Imagineer"),
    (0x9D,	"Banpresto"),
    (0x9F,	"Nova"),
    (0xA1,	"Hori Electric"),
    (0xA2,	"Bandai"),
    (0xA4,	"Konami"),
    (0xA6,	"Kawada"),
    (0xA7,	"Takara"),
    (0xA9,	"Technos Japan"),
    (0xAA,	"Broderbund"),
    (0xAC,	"Toei Animation"),
    (0xAD,	"Toho"),
    (0xAF,	"Namco"),
    (0xB0,	"Acclaim Entertainment"),
    (0xB1,	"ASCII Corporation or Nexsoft"),
    (0xB2,	"Bandai"),
    (0xB4,	"Square Enix"),
    (0xB6,	"HAL Laboratory"),
    (0xB7,	"SNK"),
    (0xB9,	"Pony Canyon"),
    (0xBA,	"Culture Brain"),
    (0xBB,	"Sunsoft"),
    (0xBD,	"Sony Imagesoft"),
    (0xBF,	"Sammy Corporation"),
    (0xC0,	"Taito"),
    (0xC2,	"Kemco"),
    (0xC3,	"Square"),
    (0xC4,	"Tokuma Shoten"),
    (0xC5,	"Data East"),
    (0xC6,	"Tonkin House"),
    (0xC8,	"Koei"),
    (0xC9,	"UFL"),
    (0xCA,	"Ultra Games"),
    (0xCB,	"VAP, Inc."),
    (0xCC,	"Use Corporation"),
    (0xCD,	"Meldac"),
    (0xCE,	"Pony Canyon"),
    (0xCF,	"Angel"),
    (0xD0,	"Taito"),
    (0xD1,	"SOFEL (Software Engineering Lab)"),
    (0xD2,	"Quest"),
    (0xD3,	"Sigma Enterprises"),
    (0xD4,	"ASK Kodansha Co."),
    (0xD6,	"Naxat Soft14"),
    (0xD7,	"Copya System"),
    (0xD9,	"Banpresto"),
    (0xDA,	"Tomy"),
    (0xDB,	"LJN"),
    (0xDD,	"Nippon Computer Systems"),
    (0xDE,	"Human Ent."),
    (0xDF,	"Altron"),
    (0xE0,	"Jaleco"),
    (0xE1,	"Towa Chiki"),
    (0xE2,	"Yutaka # Needs more info"),
    (0xE3,	"Varie"),
    (0xE5,	"Epoch"),
    (0xE7,	"Athena"),
    (0xE8,	"Asmik Ace Entertainment"),
    (0xE9,	"Natsume"),
    (0xEA,	"King Records"),
    (0xEB,	"Atlus"),
    (0xEC,	"Epic/Sony Records"),
    (0xEE,	"IGS"),
    (0xF0,	"A Wave"),
    (0xF3,	"Extreme Entertainment"),
    (0xFF,	"LJN"),
];