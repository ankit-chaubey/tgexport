/*
 * Copyright (c) 2026 Ankit Chaubey <ankitchaubey.dev@gmail.com>
 * https://github.com/ankit-chaubey
 *
 * Project: tgexport
 * Repository: https://github.com/ankit-chaubey/tgexport
 *
 * Licensed under either the Apache License, Version 2.0
 * (LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0)
 * or the MIT License (LICENSE-MIT or https://opensource.org/licenses/MIT),
 * at your option.
 */

//! Phone number formatting, a port of `Countries::format` from Telegram Desktop
//! (itself ported from TDLib) over the built in country list.

struct Code {
    code: &'static str,
    prefixes: &'static [&'static str],
    patterns: &'static [&'static str],
}

const CODES: &[Code] = &[
    Code { code: "376", prefixes: &[], patterns: &["XX XX XX"] },
    Code { code: "971", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "93", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "1268", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "1264", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "355", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "374", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "244", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "54", prefixes: &[], patterns: &[] },
    Code { code: "1684", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "43", prefixes: &[], patterns: &["X XXXXXXXX"] },
    Code { code: "61", prefixes: &[], patterns: &["X XXXX XXXX"] },
    Code { code: "297", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "994", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "387", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "1246", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "880", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "32", prefixes: &[], patterns: &["XXX XX XX XX"] },
    Code { code: "226", prefixes: &[], patterns: &["XX XX XX XX"] },
    Code { code: "359", prefixes: &[], patterns: &[] },
    Code { code: "973", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "257", prefixes: &[], patterns: &["XX XX XXXX"] },
    Code { code: "229", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "1441", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "673", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "591", prefixes: &[], patterns: &["X XXX XXXX"] },
    Code { code: "599", prefixes: &[], patterns: &[] },
    Code { code: "55", prefixes: &[], patterns: &["XX XXXXX XXXX"] },
    Code { code: "1242", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "975", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "267", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "375", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "501", prefixes: &[], patterns: &[] },
    Code { code: "1", prefixes: &["403"], patterns: &["XXX XXX XXXX"] },
    Code { code: "243", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "236", prefixes: &[], patterns: &["XX XX XX XX"] },
    Code { code: "242", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "41", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "225", prefixes: &[], patterns: &["XX XX XX XXXX"] },
    Code { code: "682", prefixes: &[], patterns: &[] },
    Code { code: "56", prefixes: &[], patterns: &["X XXXX XXXX"] },
    Code { code: "237", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "86", prefixes: &[], patterns: &["XXX XXXX XXXX"] },
    Code { code: "57", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "506", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "53", prefixes: &[], patterns: &["X XXX XXXX"] },
    Code { code: "238", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "599", prefixes: &["9"], patterns: &[] },
    Code { code: "357", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "420", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "49", prefixes: &[], patterns: &["XXXX XXXXXXX"] },
    Code { code: "253", prefixes: &[], patterns: &["XX XX XX XX"] },
    Code { code: "45", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "1767", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "1809", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "213", prefixes: &[], patterns: &["XXX XX XX XX"] },
    Code { code: "593", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "372", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "20", prefixes: &[], patterns: &["XX XXXX XXXX"] },
    Code { code: "291", prefixes: &[], patterns: &["X XXX XXX"] },
    Code { code: "34", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "251", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "358", prefixes: &[], patterns: &[] },
    Code { code: "679", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "500", prefixes: &[], patterns: &[] },
    Code { code: "691", prefixes: &[], patterns: &[] },
    Code { code: "298", prefixes: &[], patterns: &["XXX XXX"] },
    Code { code: "33", prefixes: &[], patterns: &["X XX XX XX XX"] },
    Code { code: "241", prefixes: &[], patterns: &["X XX XX XX"] },
    Code { code: "44", prefixes: &[], patterns: &["XXXX XXXXXX"] },
    Code { code: "1473", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "995", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "594", prefixes: &[], patterns: &[] },
    Code { code: "233", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "350", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "299", prefixes: &[], patterns: &["XXX XXX"] },
    Code { code: "220", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "224", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "590", prefixes: &[], patterns: &["XXX XX XX XX"] },
    Code { code: "240", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "30", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "502", prefixes: &[], patterns: &["X XXX XXXX"] },
    Code { code: "1671", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "245", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "592", prefixes: &[], patterns: &[] },
    Code { code: "852", prefixes: &[], patterns: &["X XXX XXXX"] },
    Code { code: "504", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "385", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "509", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "36", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "62", prefixes: &[], patterns: &["XXX XXXXXX"] },
    Code { code: "353", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "972", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "91", prefixes: &[], patterns: &["XXXXX XXXXX"] },
    Code { code: "246", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "964", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "98", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "354", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "39", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "1876", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "962", prefixes: &[], patterns: &["X XXXX XXXX"] },
    Code { code: "81", prefixes: &[], patterns: &["XX XXXX XXXX"] },
    Code { code: "254", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "996", prefixes: &[], patterns: &["XXX XXXXXX"] },
    Code { code: "855", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "686", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "269", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "1869", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "850", prefixes: &[], patterns: &[] },
    Code { code: "82", prefixes: &[], patterns: &["XX XXXX XXX"] },
    Code { code: "965", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "1345", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "7", prefixes: &["6"], patterns: &["XXX XXX XX XX"] },
    Code { code: "856", prefixes: &[], patterns: &["XX XX XXX XXX"] },
    Code { code: "961", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "1758", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "423", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "94", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "231", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "266", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "370", prefixes: &[], patterns: &["XXX XXXXX"] },
    Code { code: "352", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "371", prefixes: &[], patterns: &["XXX XXXXX"] },
    Code { code: "218", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "212", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "377", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "373", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "382", prefixes: &[], patterns: &[] },
    Code { code: "261", prefixes: &[], patterns: &["XX XX XXX XX"] },
    Code { code: "692", prefixes: &[], patterns: &[] },
    Code { code: "389", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "223", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "95", prefixes: &[], patterns: &[] },
    Code { code: "976", prefixes: &[], patterns: &["XX XX XXXX"] },
    Code { code: "853", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "1670", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "596", prefixes: &[], patterns: &[] },
    Code { code: "222", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "1664", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "356", prefixes: &[], patterns: &["XX XX XX XX"] },
    Code { code: "230", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "960", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "265", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "52", prefixes: &[], patterns: &[] },
    Code { code: "60", prefixes: &[], patterns: &["XX XXXX XXXX"] },
    Code { code: "258", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "264", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "687", prefixes: &[], patterns: &[] },
    Code { code: "227", prefixes: &[], patterns: &["XX XX XX XX"] },
    Code { code: "672", prefixes: &[], patterns: &[] },
    Code { code: "234", prefixes: &[], patterns: &["XX XXXX XXXX"] },
    Code { code: "505", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "31", prefixes: &[], patterns: &["X XX XX XX XX"] },
    Code { code: "47", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "977", prefixes: &[], patterns: &["XX XXXX XXXX"] },
    Code { code: "674", prefixes: &[], patterns: &[] },
    Code { code: "683", prefixes: &[], patterns: &[] },
    Code { code: "64", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "968", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "507", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "51", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "689", prefixes: &[], patterns: &[] },
    Code { code: "675", prefixes: &[], patterns: &[] },
    Code { code: "63", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "92", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "48", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "508", prefixes: &[], patterns: &[] },
    Code { code: "1787", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "970", prefixes: &[], patterns: &["XXX XX XXXX"] },
    Code { code: "351", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "680", prefixes: &[], patterns: &[] },
    Code { code: "595", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "974", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "262", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "40", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "381", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "7", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "250", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "966", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "677", prefixes: &[], patterns: &[] },
    Code { code: "248", prefixes: &[], patterns: &["X XX XX XX"] },
    Code { code: "249", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "46", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "65", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "247", prefixes: &[], patterns: &[] },
    Code { code: "386", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "421", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "232", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "378", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "221", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "252", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "597", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "211", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "239", prefixes: &[], patterns: &["XX XXXXX"] },
    Code { code: "503", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "1721", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "963", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "268", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "1649", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "235", prefixes: &[], patterns: &["XX XX XX XX"] },
    Code { code: "228", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "66", prefixes: &[], patterns: &["X XXXX XXXX"] },
    Code { code: "992", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "690", prefixes: &[], patterns: &[] },
    Code { code: "670", prefixes: &[], patterns: &[] },
    Code { code: "993", prefixes: &[], patterns: &["XX XXXXXX"] },
    Code { code: "216", prefixes: &[], patterns: &["XX XXX XXX"] },
    Code { code: "676", prefixes: &[], patterns: &[] },
    Code { code: "90", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "1868", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "688", prefixes: &[], patterns: &[] },
    Code { code: "886", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "255", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "380", prefixes: &[], patterns: &["XX XXX XX XX"] },
    Code { code: "256", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "1", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "598", prefixes: &[], patterns: &["X XXX XXXX"] },
    Code { code: "998", prefixes: &[], patterns: &["XX XXX XX XX"] },
    Code { code: "1784", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "58", prefixes: &[], patterns: &["XXX XXX XXXX"] },
    Code { code: "1284", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "1340", prefixes: &[], patterns: &["XXX XXXX"] },
    Code { code: "84", prefixes: &[], patterns: &[] },
    Code { code: "678", prefixes: &[], patterns: &[] },
    Code { code: "681", prefixes: &[], patterns: &[] },
    Code { code: "685", prefixes: &[], patterns: &[] },
    Code { code: "383", prefixes: &[], patterns: &["XXXX XXXX"] },
    Code { code: "967", prefixes: &[], patterns: &["XXX XXX XXX"] },
    Code { code: "27", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "260", prefixes: &[], patterns: &["XX XXX XXXX"] },
    Code { code: "263", prefixes: &[], patterns: &["XX XXX XXXX"] },
];

/// What `Ui::FormatPhone` returns for a stored phone number.
pub fn format_phone(phone: &str) -> String {
    if phone.is_empty() || phone.starts_with('0') {
        return phone.to_string();
    }
    let phone: String = phone.chars().filter(|c| *c != ' ').collect();
    let digits = phone.strip_prefix('+').unwrap_or(&phone);
    format_digits(digits)
}

fn format_digits(phone: &str) -> String {
    let mut best: Option<(&Code, usize)> = None;
    for c in CODES {
        if !phone.starts_with(c.code) {
            continue;
        }
        let rest = &phone[c.code.len()..];
        // The server list gives a country without prefixes one empty prefix,
        // which is what Telegram Desktop works with after it loads the list.
        let prefixes: &[&str] = if c.prefixes.is_empty() { &[""] } else { c.prefixes };
        for p in prefixes {
            let length = c.code.len() + p.len();
            if rest.starts_with(p) && length > best.map(|b| b.1).unwrap_or(0) {
                best = Some((c, length));
            }
        }
    }
    let Some((code, _)) = best else {
        return phone.to_string();
    };
    let part: Vec<char> = phone[code.code.len()..].chars().collect();
    let mut formatted: String = part.iter().collect();
    let mut max_matched = 0usize;
    for pattern in code.patterns {
        let pattern: Vec<char> = pattern.chars().collect();
        let mut result = String::new();
        let mut pos = 0usize;
        let mut failed = false;
        let mut matched = 0usize;
        for &c in &part {
            while pos < pattern.len() && pattern[pos] != 'X' && !pattern[pos].is_ascii_digit() {
                result.push(pattern[pos]);
                pos += 1;
            }
            if pos >= pattern.len() || pattern[pos] == 'X' {
                pos += 1;
                result.push(c);
            } else if c == pattern[pos] {
                matched += 1;
                pos += 1;
                result.push(c);
            } else {
                failed = true;
                break;
            }
        }
        if !failed && matched >= max_matched {
            max_matched = matched;
            formatted = result;
        }
    }
    format!("+{} {}", code.code, formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_follow_the_country_pattern() {
        assert_eq!(format_phone("919876543210"), "+91 98765 43210");
        assert_eq!(format_phone("+7 912 345 6789"), "+7 912 345 6789");
    }

    #[test]
    fn unknown_and_local_numbers_stay_as_they_are() {
        assert_eq!(format_phone(""), "");
        assert_eq!(format_phone("0123"), "0123");
    }
}
