use x86_64::instructions::port::Port;

// CMOS/RTC ports: 0x70 (address), 0x71 (data)
unsafe fn read_cmos(reg: u8) -> u8 {
    let mut addr = Port::<u8>::new(0x70);
    let mut data = Port::<u8>::new(0x71);
    addr.write(reg);
    data.read()
}

fn bcd_to_bin(val: u8) -> u8 {
    (val & 0x0F) + ((val >> 4) * 10)
}

// UTC offset (Turkiye = +3)
const UTC_OFFSET: i32 = 3;

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(month: i32, year: i32) -> i32 {
    match month {
        1 => 31, 2 => if is_leap(year) { 29 } else { 28 },
        3 => 31, 4 => 30, 5 => 31, 6 => 30,
        7 => 31, 8 => 31, 9 => 30, 10 => 31, 11 => 30, 12 => 31,
        _ => 30,
    }
}

// freeze date/time: (hour, minute, day, month, year)
pub fn read() -> (u8, u8, u8, u8, u8, u8) {
    unsafe {
        // Wait until the update is finished (bit 7 of reg 0x0A)
        while read_cmos(0x0A) & 0x80 != 0 {}

        let raw_sec = read_cmos(0x00);
        let raw_min = read_cmos(0x02);
        let raw_hour = read_cmos(0x04);
        let raw_day = read_cmos(0x07);
        let raw_month = read_cmos(0x08);
        let raw_year = read_cmos(0x09);

        // Format control (reg 0x0B bit 2: 0=BCD, 1=binary)
        let status_b = read_cmos(0x0B);
        let is_bcd = status_b & 0x04 == 0;

        // First translate it to binary (no offset)
        let (sec, min, hour, day, month, year) = if is_bcd {
            (bcd_to_bin(raw_sec), bcd_to_bin(raw_min), bcd_to_bin(raw_hour),
             bcd_to_bin(raw_day), bcd_to_bin(raw_month), bcd_to_bin(raw_year))
        } else {
            (raw_sec, raw_min, raw_hour, raw_day, raw_month, raw_year)
        };

        // apply offset 
        // TODO: Read from offset register (select time freely)
        let mut h = hour as i32 + UTC_OFFSET;
        let mut d = day as i32;
        let mut mo = month as i32;
        let mut y = 2000 + year as i32;

        while h >= 24 {
            h -= 24;
            d += 1;
            if d > days_in_month(mo, y) {
                d = 1;
                mo += 1;
                if mo > 12 { mo = 1; y += 1; }
            }
        }
        while h < 0 { // negative ofset, for (UTC-x)
            h += 24;
            d -= 1;
            if d < 1 {
                mo -= 1;
                if mo < 1 { mo = 12; y -= 1; }
                d = days_in_month(mo, y);
            }
        }

        (h as u8, min, sec, d as u8, mo as u8, (y % 100) as u8)
    }
}