// Rusty Aero tema paleti - tum renkler tek yerde

// Arka plan
pub const BG_TOP: u32 = 0x00501808;      // koyu bordo
pub const BG_BOTTOM: u32 = 0x00A03810;   // sicak turuncu

// Pencere
pub const WINDOW_BODY: u32 = 0x00F5F0EE;  // beyazimsi govde
pub const WINDOW_BORDER: u32 = 0x00D86018; // turuncu kenarlik

// Baslik cubugu (glossy)
pub const TITLE_TOP: u32 = 0x00FF8020;     // parlak turuncu
pub const TITLE_BOTTOM: u32 = 0x00C84810;  // koyu turuncu
pub const TITLE_TEXT: u32 = 0x00FFFFFF;    // beyaz

// Aktif olmayan pencere basligi (soluk)
pub const TITLE_INACTIVE_TOP: u32 = 0x00B0907F;
pub const TITLE_INACTIVE_BOTTOM: u32 = 0x00908070;

// Butonlar
pub const CLOSE_TOP: u32 = 0x00F04020;     // kirmizi kapat
pub const CLOSE_BOTTOM: u32 = 0x00A01808;
pub const BUTTON_TEXT: u32 = 0x00FFFFFF;

// Taskbar
pub const TASKBAR_TOP: u32 = 0x00381008;
pub const TASKBAR_BOTTOM: u32 = 0x00200804;
pub const TASKBAR_HIGHLIGHT: u32 = 0x00FF8020;
pub const TASKBAR_HEIGHT: usize = 44;

// Start butonu
pub const START_TOP: u32 = 0x00FF8020;
pub const START_BOTTOM: u32 = 0x00C84810;

// Olculer
pub const TITLE_HEIGHT: usize = 36;
pub const CORNER_RADIUS: usize = 8;

// desktop.rs ve taskbar.rs icin eksik sabitler
pub const BG_MID: u32 = 0x00701C08;       // orta bordo-turuncu (arka plan gecisi)
pub const ICON_TEXT: u32 = 0x00FFFFFF;    // ikon etiketi beyaz
pub const START_TEXT: u32 = 0x00FFFFFF;   // baslat yazisi beyaz